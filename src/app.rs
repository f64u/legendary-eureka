use std::sync::Arc;

use nalgebra::{ComplexField, Vector, Vector3};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        PrimaryCommandBufferAbstract, RenderPassBeginInfo, SubpassContents,
    },
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
    },
    format::Format,
    image::{view::ImageView, ImageAccess, ImageDimensions, ImmutableImage, SwapchainImage},
    memory::allocator::{FreeListAllocator, GenericMemoryAllocator, StandardMemoryAllocator},
    pipeline::{
        graphics::{
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            rasterization::{PolygonMode, RasterizationState},
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline, Pipeline, PipelineBindPoint,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo},
    swapchain::{
        acquire_next_image, AcquireError, SwapchainCreateInfo, SwapchainCreationError,
        SwapchainPresentInfo,
    },
    sync::{self, FlushError, GpuFuture},
};
use winit::window::Window;

use crate::{
    camera::Camera,
    cell::{chunk::HFVertex, tile::Tile},
    map::Map,
    window_state::WindowState,
};

pub(crate) mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
        #version 460

        layout(location = 0) in vec3 position;
        layout(location = 1) in vec3 color;
        layout(location = 2) in vec2 txt_coord;
        layout(location = 3) in float morph_delta;

        layout(set = 0, binding = 0) uniform WorldObject {
            mat4 model;
            mat4 view;
            mat4 proj;
        } world;

        layout(location = 0) out vec3 v_color;
        layout(location = 1) out vec2 f_txt_coord;

        void main() {
            gl_Position = world.proj * world.view * world.model * vec4(position, 1.0);
            v_color = color;
            f_txt_coord = txt_coord;
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
        layout(location = 1) in vec2 txt_coord;

        layout(location = 0) out vec4 f_color;
        //layout(location = 1) out vec4 t_color;

        layout(set = 1, binding = 0) uniform sampler2D tex;

        void main() {
            f_color = texture(tex, txt_coord);
            // f_color = vec4(v_color, 1.0);
            //t_color = texture(tex, txt_coord);
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
    pub memory_allocator: GenericMemoryAllocator<Arc<FreeListAllocator>>,
    pub descriptor_set: Arc<PersistentDescriptorSet>,
    pub world_uniform_buffer: Arc<CpuAccessibleBuffer<vs::ty::WorldObject>>,
    pub camera: Camera,
    pub situation: Situation,
}

pub struct Situation {
    vertex_buffers: Vec<Arc<CpuAccessibleBuffer<[HFVertex]>>>,
    index_buffers: Vec<Arc<CpuAccessibleBuffer<[u16]>>>,
    images: Vec<Arc<ImageView<ImmutableImage>>>,
}

impl Situation {
    fn new(
        memory_allocator: &GenericMemoryAllocator<Arc<FreeListAllocator>>,
        tiles: Vec<&Tile>,
        images: Vec<Arc<ImageView<ImmutableImage>>>,
        camera: &Camera,
    ) -> Self {
        // let frustum = dbg!(dbg!(camera).frustum());
        let level = tiles[0].level;
        let chunks = tiles
            .into_iter()
            // .filter(
            //     |tile| match frustum.intersect(tile.bbox.as_ref().unwrap()) {
            //         crate::geometry::IntersectionStatus::Outside => false,
            //         _ => true,
            //     },
            // )
            .map(|tile| {
                let pos = tile.bbox.as_ref().unwrap().max;
                (&tile.chunk, [pos.x, pos.z])
            })
            .collect::<Vec<_>>();

        const COLORS: [[f32; 3]; 4] = [
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ];

        let vertex_buffers = chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| {
                let (chunk, chunk_pos) = chunk;
                CpuAccessibleBuffer::from_iter(
                    memory_allocator,
                    BufferUsage {
                        vertex_buffer: true,
                        ..Default::default()
                    },
                    false,
                    chunk.vertices.iter().map(move |v| {
                        let coords = [
                            (v.position[0] - chunk_pos[0] as f32) / 2i32.pow(5 * 2 - level) as f32,
                            (v.position[2] - chunk_pos[1] as f32) / 2i32.pow(5 * 2 - level) as f32,
                        ];
                        v.with_color_and_coords(COLORS[i % 4], coords)
                    }),
                )
                .unwrap()
            })
            .collect();

        let index_buffers = chunks
            .iter()
            .map(|(chunk, _)| {
                CpuAccessibleBuffer::from_iter(
                    memory_allocator,
                    BufferUsage {
                        index_buffer: true,
                        ..Default::default()
                    },
                    false,
                    chunk.indices.iter().copied(),
                )
                .unwrap()
            })
            .collect();

        Self {
            vertex_buffers,
            index_buffers,
            images,
        }
    }
}

pub enum SwapchainState {
    SubOptimal,
    Dirty,
    Good,
}

impl App {
    pub fn new(window_state: WindowState, map: Map) -> Self {
        let memory_allocator = StandardMemoryAllocator::new_default(window_state.device.clone());

        let camera = Camera::default();

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

        let sampler = Sampler::new(
            window_state.device.clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [SamplerAddressMode::Repeat; 3],
                ..Default::default()
            },
        )
        .unwrap();

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
            .rasterization_state(RasterizationState {
                // polygon_mode: PolygonMode::Line,
                ..Default::default()
            })
            .with_auto_layout(window_state.device.clone(), |layout_create_infos| {
                let create_info = &mut layout_create_infos[1];
                let binding = create_info.bindings.get_mut(&0).unwrap();
                create_info.push_descriptor = true;
                binding.immutable_samplers = vec![sampler];
            })
            // .build(window_state.device.clone())
            .unwrap();

        let mut uploads = AutoCommandBufferBuilder::primary(
            &command_buffer_allocator,
            window_state.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let cell = &map.cells[0][0];
        let tiles = cell.tree.items_at_level(0);

        let images = tiles
            .iter()
            .map(|tile| {
                let texture = tile.texture.as_ref().unwrap();
                let image = ImmutableImage::from_iter(
                    &memory_allocator,
                    texture.image.clone(),
                    ImageDimensions::Dim2d {
                        width: texture.size,
                        height: texture.size,
                        array_layers: 1,
                    },
                    vulkano::image::MipmapsCount::One,
                    Format::R8G8B8A8_SRGB,
                    &mut uploads,
                )
                .unwrap();

                ImageView::new_default(image).unwrap()
            })
            .collect();

        let situation = Situation::new(&memory_allocator, tiles, images, &camera);

        let layout = pipeline.layout().set_layouts().get(0).unwrap();
        let descriptor_set = PersistentDescriptorSet::new(
            &descriptor_set_allocator,
            layout.clone(),
            [WriteDescriptorSet::buffer(0, world_uniform_buffer.clone())],
        )
        .unwrap();

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

        let previous_frame_end = Some(
            uploads
                .build()
                .unwrap()
                .execute(window_state.queue.clone())
                .unwrap()
                .boxed(),
        );

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
            memory_allocator,
            descriptor_set,
            world_uniform_buffer,
            camera,
            situation,
        }
    }

    pub fn camera_updated(&mut self) {
        if let Ok(mut world) = self.world_uniform_buffer.write() {
            *world = self.camera.world_object(self.map.scale())
        }

        // self.situation = Situation::new(
        //     &self.memory_allocator,
        //     self.map.cells[0][0].tree.items_at_level(1),
        //     &self.camera,
        // )
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
            .bind_pipeline_graphics(self.pipeline.clone())
            .set_viewport(0, [self.viewport.clone()]);

        for ((vertex_buffer, index_buffer), image) in self
            .situation
            .vertex_buffers
            .iter()
            .zip(self.situation.index_buffers.iter())
            .zip(self.situation.images.iter())
        {
            builder
                .push_descriptor_set(
                    PipelineBindPoint::Graphics,
                    self.pipeline.layout().clone(),
                    1,
                    [WriteDescriptorSet::image_view(0, image.clone())],
                )
                .bind_vertex_buffers(0, vertex_buffer.clone())
                .bind_index_buffer(index_buffer.clone())
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    self.pipeline.layout().clone(),
                    0,
                    self.descriptor_set.clone(),
                )
                .draw_indexed(index_buffer.len() as u32, 1, 0, 0, 0)
                .unwrap();
        }
        builder.end_render_pass().unwrap();

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
