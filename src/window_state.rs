use std::sync::Arc;

use vulkano::{
    device::{
        physical::PhysicalDeviceType, Device, DeviceCreateInfo, DeviceExtensions, Features, Queue,
        QueueCreateInfo, QueueFlags,
    },
    image::{ImageUsage, SwapchainImage},
    instance::{Instance, InstanceCreateInfo},
    swapchain::{Surface, Swapchain, SwapchainCreateInfo},
    VulkanLibrary,
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

pub struct WindowState {
    pub instance: Arc<Instance>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub surface: Arc<Surface>,
    pub swapchain: Arc<Swapchain>,
    pub swapchain_images: Vec<Arc<SwapchainImage>>,
}

impl WindowState {
    fn create_vulkan_instance() -> Arc<Instance> {
        let library = VulkanLibrary::new().unwrap();
        let required_extentions = vulkano_win::required_extensions(&library);

        Instance::new(
            library,
            InstanceCreateInfo {
                enabled_extensions: required_extentions,
                enumerate_portability: true,
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn create_surface(
        title: String,
        event_loop: &EventLoop<()>,
        instance: Arc<Instance>,
    ) -> Arc<Surface> {
        WindowBuilder::new()
            .with_title(title)
            .build_vk_surface(event_loop, instance.clone())
            .unwrap()
    }

    fn get_device_and_queue(
        instance: Arc<Instance>,
        surface: Arc<Surface>,
    ) -> (Arc<Device>, Arc<Queue>) {
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            khr_push_descriptor: true,
            ..Default::default()
        };

        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .unwrap()
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags.intersects(&QueueFlags {
                            graphics: true,
                            ..Default::default()
                        }) && p.surface_support(i as u32, &surface).unwrap_or(false)
                    })
                    .map(|i| (p, i as u32))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
                _ => 5,
            })
            .expect("error finding queue.");

        let (device, mut queues) = Device::new(
            physical_device,
            DeviceCreateInfo {
                enabled_features: Features {
                    fill_mode_non_solid: true,
                    ..Default::default()
                },
                enabled_extensions: device_extensions,
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .unwrap();

        let queue = queues.next().unwrap();

        (device, queue)
    }

    fn create_swapchain(
        device: Arc<Device>,
        surface: Arc<Surface>,
    ) -> (Arc<Swapchain>, Vec<Arc<SwapchainImage>>) {
        let surface_capabilities = device
            .physical_device()
            .surface_capabilities(&surface, Default::default())
            .unwrap();

        let image_format = Some(
            device
                .physical_device()
                .surface_formats(&surface, Default::default())
                .unwrap()[0]
                .0,
        );

        let window = surface.object().unwrap().downcast_ref::<Window>().unwrap();

        Swapchain::new(
            device.clone(),
            surface.clone(),
            SwapchainCreateInfo {
                min_image_count: surface_capabilities.min_image_count,
                image_format,
                image_extent: window.inner_size().into(),
                image_usage: ImageUsage {
                    color_attachment: true,
                    ..Default::default()
                },
                composite_alpha: surface_capabilities
                    .supported_composite_alpha
                    .iter()
                    .next()
                    .unwrap(),
                ..Default::default()
            },
        )
        .unwrap()
    }

    pub fn create(title: String) -> (Self, EventLoop<()>) {
        let event_loop = EventLoop::new();
        let instance = Self::create_vulkan_instance();
        let surface = Self::create_surface(title, &event_loop, instance.clone());
        let (device, queue) = Self::get_device_and_queue(instance.clone(), surface.clone());
        let (swapchain, images) = Self::create_swapchain(device.clone(), surface.clone());

        (
            Self {
                instance,
                device,
                queue,
                surface,
                swapchain,
                swapchain_images: images,
            },
            event_loop,
        )
    }
}
