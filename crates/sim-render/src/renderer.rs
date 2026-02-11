// Vulkano instance/device/swapchain/renderpass — Phase 3 implementation.

use std::sync::Arc;
use vulkano::{
    device::{
        physical::PhysicalDeviceType, Device, DeviceCreateInfo, DeviceExtensions, Queue,
        QueueCreateInfo, QueueFlags,
    },
    image::{view::ImageView, Image, ImageUsage},
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    swapchain::{
        self, Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo,
    },
    sync::{self, GpuFuture},
    Validated, VulkanError, VulkanLibrary,
};
use winit::{
    dpi::LogicalSize,
    event_loop::ActiveEventLoop,
    window::Window,
};

pub struct Renderer {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub surface: Arc<Surface>,
    pub swapchain: Arc<Swapchain>,
    pub images: Vec<Arc<Image>>,
    pub image_views: Vec<Arc<ImageView>>,
    pub window: Arc<Window>,
    pub recreate_swapchain: bool,
    previous_frame_end: Option<Box<dyn GpuFuture>>,
}

impl Renderer {
    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        // --- Vulkan library + instance ---
        let library = VulkanLibrary::new().expect("no Vulkan library found");
        let required_extensions =
            Surface::required_extensions(event_loop).expect("failed to get required extensions");
        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                enabled_extensions: required_extensions,
                ..Default::default()
            },
        )
        .expect("failed to create Vulkan instance");

        // --- Window + surface ---
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Air-Sim — Expansion Chamber Muffler Simulator")
                        .with_inner_size(LogicalSize::new(1280u32, 800u32)),
                )
                .expect("failed to create window"),
        );
        let surface = Surface::from_window(instance.clone(), window.clone())
            .expect("failed to create surface");

        // --- Physical device selection ---
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };
        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .expect("failed to enumerate physical devices")
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags.intersects(QueueFlags::GRAPHICS)
                            && p.surface_support(i as u32, &surface).unwrap_or(false)
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
            .expect("no suitable physical device found");

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );

        // --- Logical device + queue ---
        let (device, mut queues) = Device::new(
            physical_device.clone(),
            DeviceCreateInfo {
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                enabled_extensions: device_extensions,
                ..Default::default()
            },
        )
        .expect("failed to create logical device");
        let queue = queues.next().expect("no queue available");

        // --- Swapchain ---
        let surface_capabilities = physical_device
            .surface_capabilities(&surface, Default::default())
            .expect("failed to query surface capabilities");
        let image_format = physical_device
            .surface_formats(&surface, Default::default())
            .expect("failed to query surface formats")[0]
            .0;
        let window_size = window.inner_size();
        let (swapchain, images) = Swapchain::new(
            device.clone(),
            surface.clone(),
            SwapchainCreateInfo {
                min_image_count: surface_capabilities.min_image_count.max(2),
                image_format,
                image_extent: [window_size.width, window_size.height],
                image_usage: ImageUsage::COLOR_ATTACHMENT,
                composite_alpha: surface_capabilities
                    .supported_composite_alpha
                    .into_iter()
                    .next()
                    .expect("no composite alpha mode"),
                ..Default::default()
            },
        )
        .expect("failed to create swapchain");

        let image_views = images
            .iter()
            .map(|image| ImageView::new_default(image.clone()).unwrap())
            .collect::<Vec<_>>();

        let previous_frame_end = Some(sync::now(device.clone()).boxed());

        Renderer {
            device,
            queue,
            surface,
            swapchain,
            images,
            image_views,
            window,
            recreate_swapchain: false,
            previous_frame_end,
        }
    }

    pub fn swapchain_format(&self) -> vulkano::format::Format {
        self.swapchain.image_format()
    }

    pub fn recreate_swapchain(&mut self) {
        let window_size = self.window.inner_size();
        if window_size.width == 0 || window_size.height == 0 {
            return;
        }
        let (new_swapchain, new_images) = self
            .swapchain
            .recreate(SwapchainCreateInfo {
                image_extent: [window_size.width, window_size.height],
                ..self.swapchain.create_info()
            })
            .expect("failed to recreate swapchain");

        self.swapchain = new_swapchain;
        self.images = new_images;
        self.image_views = self
            .images
            .iter()
            .map(|image| ImageView::new_default(image.clone()).unwrap())
            .collect();
        self.recreate_swapchain = false;
    }

    /// Acquire the next swapchain image. Returns the image index and acquire future,
    /// or None if the swapchain needs recreation.
    pub fn begin_frame(
        &mut self,
    ) -> Option<(u32, swapchain::SwapchainAcquireFuture)> {
        // Clean up finished GPU work
        if let Some(ref mut future) = self.previous_frame_end {
            future.cleanup_finished();
        }

        if self.recreate_swapchain {
            self.recreate_swapchain();
        }

        let (image_index, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.swapchain.clone(), None)
                .map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return None;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };
        if suboptimal {
            self.recreate_swapchain = true;
        }
        Some((image_index, acquire_future))
    }

    /// Present the rendered image. Takes the acquire future joined with render work.
    pub fn end_frame(&mut self, after_future: Box<dyn GpuFuture>) {
        self.previous_frame_end = Some(after_future);
    }

    /// Get the previous frame end future, for joining with new GPU work.
    pub fn take_previous_frame_end(&mut self) -> Box<dyn GpuFuture> {
        self.previous_frame_end
            .take()
            .unwrap_or_else(|| sync::now(self.device.clone()).boxed())
    }

    pub fn present(
        &self,
        after_future: Box<dyn GpuFuture>,
        image_index: u32,
    ) -> Box<dyn GpuFuture> {
        let future = after_future
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(
                    self.swapchain.clone(),
                    image_index,
                ),
            )
            .then_signal_fence_and_flush();

        match future.map_err(Validated::unwrap) {
            Ok(future) => future.boxed(),
            Err(VulkanError::OutOfDate) => {
                sync::now(self.device.clone()).boxed()
            }
            Err(e) => {
                eprintln!("failed to flush future: {e}");
                sync::now(self.device.clone()).boxed()
            }
        }
    }
}
