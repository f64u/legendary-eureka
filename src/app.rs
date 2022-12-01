use std::sync::Arc;

use nalgebra::{Point3, Vector3};

use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        RenderPassBeginInfo, SubpassContents,
    },
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
    },
    image::{view::ImageView, ImageAccess, SwapchainImage},
    memory::allocator::StandardMemoryAllocator,
    pipeline::{
        graphics::{
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline, Pipeline, PipelineBindPoint,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    swapchain::{
        acquire_next_image, AcquireError, SwapchainCreateInfo, SwapchainCreationError,
        SwapchainPresentInfo,
    },
    sync::{self, FlushError, GpuFuture},
};
use winit::window::Window;

use crate::{camera::Camera, cell::chunk::HFVertex, map::Map, vulkan_state::WindowState};

pub(crate) mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
        #version 460

        layout(location = 0) in vec3 position;
        layout(location = 1) in float morph_delta;

        layout(set = 0, binding = 0) uniform WorldObject {
            mat4 model;
            mat4 view;
            mat4 proj;
        } world;

        layout(location = 0) out vec3 v_color;

        void main() {
            gl_Position = world.proj * world.view * world.model * vec4(position, 1.0);
            v_color = vec3(1.0, 0.0, 0.0);
        }
    ",
    types_meta: {
        use bytemuck::{Pod, Zeroable};

        #[derive(Clone, Copy, Default, Zeroable, Pod)]
    }
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
        #version 460

        layout(location = 0) in vec3 v_color; 

        layout(location = 0) out vec4 f_color;

        void main() {
            f_color = vec4(v_color, 1.0);
        }
    "
    }
}

pub struct App {
    pub map: Map,
    pub window_state: WindowState,
    pub previous_frame_end: Option<Box<dyn GpuFuture>>,
    pub render_pass: Arc<RenderPass>,
    pub pipeline: Arc<GraphicsPipeline>,
    pub framebuffers: Vec<Arc<Framebuffer>>,
    pub viewport: Viewport,
    pub command_buffer_allocator: StandardCommandBufferAllocator,
    pub descriptor_set_allocator: StandardDescriptorSetAllocator,
    pub vertex_buffer: Arc<CpuAccessibleBuffer<[HFVertex]>>,
    pub index_buffer: Arc<CpuAccessibleBuffer<[u16]>>,
    pub descriptor_set: Arc<PersistentDescriptorSet>,
    pub world_uniform_buffer: Arc<CpuAccessibleBuffer<vs::ty::WorldObject>>,
    pub camera: Camera,
}

pub enum SwapchainState {
    SubOptimal,
    Dirty,
    Good,
}

impl App {
    pub fn new(window_state: WindowState, map: Map) -> Self {
        let chunk = &map.cells[0][0].levels[0][0].chunk;

        let memory_allocator = StandardMemoryAllocator::new_default(window_state.device.clone());

        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            &memory_allocator,
            BufferUsage {
                vertex_buffer: true,
                ..Default::default()
            },
            false,
            chunk.vertices.iter().cloned(),
        )
        .unwrap();

        let index_buffer = CpuAccessibleBuffer::from_iter(
            &memory_allocator,
            BufferUsage {
                index_buffer: true,
                ..Default::default()
            },
            false,
            chunk.indices.iter().copied(),
        )
        .unwrap();

        let camera = Camera {
            pos: Point3::new(0.0, 13.0, 0.0),
            target: Point3::new(0.0, 0.0, 0.0),
            up: -Vector3::z(),
            near_z: 0.1,
            far_z: 1000.0,
            asepect_ratio: 16.0 / 9.0,
            fov: 60.0,
            error_factor: 0.1,
            width: 200,
        };

        let world_uniform_buffer = CpuAccessibleBuffer::from_data(
            &memory_allocator,
            BufferUsage {
                uniform_buffer: true,
                ..Default::default()
            },
            false,
            camera.world_object(map.scale()),
        )
        .unwrap();

        let vs = vs::load(window_state.device.clone()).unwrap();
        let fs = fs::load(window_state.device.clone()).unwrap();

        let render_pass = vulkano::single_pass_renderpass!(
            window_state.device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: window_state.swapchain.image_format(),
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
        .unwrap();

        let command_buffer_allocator =
            StandardCommandBufferAllocator::new(window_state.device.clone(), Default::default());

        let descriptor_set_allocator =
            StandardDescriptorSetAllocator::new(window_state.device.clone());

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };

        let framebuffers = _window_size_dependent_setup(
            &window_state.swapchain_images,
            render_pass.clone(),
            &mut viewport,
        );

        let previous_frame_end = Some(sync::now(window_state.device.clone()).boxed());

        let pipeline = GraphicsPipeline::start()
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .vertex_input_state(BuffersDefinition::new().vertex::<HFVertex>())
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState {
                topology: vulkano::pipeline::PartialStateMode::Fixed(
                    PrimitiveTopology::TriangleStrip,
                ),
                ..Default::default()
            })
            .build(window_state.device.clone())
            .unwrap();

        let layout = pipeline.layout().set_layouts().get(0).unwrap();
        let descriptor_set = PersistentDescriptorSet::new(
            &descriptor_set_allocator,
            layout.clone(),
            [WriteDescriptorSet::buffer(0, world_uniform_buffer.clone())],
        )
        .unwrap();

        Self {
            map,
            window_state,
            previous_frame_end,
            render_pass,
            pipeline,
            framebuffers,
            viewport,
            command_buffer_allocator,
            descriptor_set_allocator,
            vertex_buffer,
            index_buffer,
            descriptor_set,
            world_uniform_buffer,
            camera,
        }
    }

    pub fn reupload_world_data(&self) {
        if let Ok(mut world) = self.world_uniform_buffer.write() {
            *world = self.camera.world_object(self.map.scale())
        }
    }

    pub fn recreate_swapchain(&mut self) {
        let window = self
            .window_state
            .surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap();
        let dimensions = window.inner_size();
        if dimensions.width == 0 || dimensions.height == 0 {
            return;
        }
        let (new_swapchain, new_images) =
            match self.window_state.swapchain.recreate(SwapchainCreateInfo {
                image_extent: dimensions.into(),
                ..self.window_state.swapchain.create_info()
            }) {
                Ok(r) => r,
                Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
            };

        self.window_state.swapchain = new_swapchain;
        self.framebuffers =
            _window_size_dependent_setup(&new_images, self.render_pass.clone(), &mut self.viewport);
    }

    pub fn draw(&mut self) -> SwapchainState {
        let mut state = SwapchainState::Good;
        let (image_index, suboptimal, acquire_future) =
            match acquire_next_image(self.window_state.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    return SwapchainState::Dirty;
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };

        if suboptimal {
            state = SwapchainState::SubOptimal;
        }

        let mut builder = AutoCommandBufferBuilder::primary(
            &self.command_buffer_allocator,
            self.window_state.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some([0.0, 0.0, 0.0, 1.0].into())],
                    ..RenderPassBeginInfo::framebuffer(
                        self.framebuffers[image_index as usize].clone(),
                    )
                },
                SubpassContents::Inline,
            )
            .unwrap()
            .set_viewport(0, [self.viewport.clone()])
            .bind_pipeline_graphics(self.pipeline.clone())
            .bind_vertex_buffers(0, self.vertex_buffer.clone())
            .bind_index_buffer(self.index_buffer.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                self.descriptor_set.clone(),
            )
            .draw_indexed(self.index_buffer.len() as u32, 1, 0, 0, 0)
            .unwrap()
            .end_render_pass()
            .unwrap();

        let command_buffer = builder.build().unwrap();

        let future = self
            .previous_frame_end
            .take()
            .unwrap()
            .join(acquire_future)
            .then_execute(self.window_state.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(
                self.window_state.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(
                    self.window_state.swapchain.clone(),
                    image_index,
                ),
            )
            .then_signal_fence_and_flush();
        match future {
            Ok(future) => {
                self.previous_frame_end = Some(future.boxed());
            }
            Err(FlushError::OutOfDate) => {
                state = SwapchainState::Dirty;
                self.previous_frame_end = Some(sync::now(self.window_state.device.clone()).boxed());
            }
            Err(e) => {
                panic!("Failed to flush future: {:?}", e);
            }
        }
        return state;
    }
}

fn _window_size_dependent_setup(
    images: &[Arc<SwapchainImage>],
    render_pass: Arc<RenderPass>,
    viewport: &mut Viewport,
) -> Vec<Arc<Framebuffer>> {
    let dimensions = images[0].dimensions().width_height();
    viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];

    images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone()).unwrap();
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>()
}
