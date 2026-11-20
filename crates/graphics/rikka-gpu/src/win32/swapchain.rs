use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::image::{ImageAccess, color_subresource_range};
use crate::error::{Error, Result};
use crate::win32::surface::{READABLE_USAGE, Surface, presenting};

const FORMATS: [vk::Format; 2] = [vk::Format::B8G8R8A8_UNORM, vk::Format::R8G8B8A8_UNORM];

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwapchainConfig {
    pub extent: vk::Extent2D,
    pub images: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acquire {
    Image { index: u32, suboptimal: bool },
    Timeout,
    Outdated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Current,
    Suboptimal,
    Outdated,
}

#[derive(Debug)]
pub struct SwapchainImage {
    image: vk::Image,
    view: vk::ImageView,
    access: ImageAccess,
    acquired: vk::Semaphore,
    rendered: vk::Semaphore,
}

impl SwapchainImage {
    pub fn handle(&self) -> vk::Image {
        self.image
    }

    pub fn view(&self) -> vk::ImageView {
        self.view
    }

    pub fn access(&self) -> ImageAccess {
        self.access
    }

    pub fn set_access(&mut self, access: ImageAccess) {
        self.access = access;
    }

    pub fn acquired(&self) -> vk::Semaphore {
        self.acquired
    }

    pub fn rendered(&self) -> vk::Semaphore {
        self.rendered
    }
}

pub struct Swapchain {
    device: Arc<RenderDevice>,
    id: u64,
    handle: vk::SwapchainKHR,
    config: SwapchainConfig,
    format: vk::Format,
    readable: bool,
    images: Vec<SwapchainImage>,
    spare: vk::Semaphore,
}

impl Swapchain {
    pub fn new(
        surface: &Surface,
        config: SwapchainConfig,
        old: Option<Swapchain>,
    ) -> Result<Swapchain> {
        let device = surface.device().clone();
        let win32 = presenting(&device)?;
        let capabilities = surface.capabilities()?;
        let format = surface
            .formats()?
            .into_iter()
            .filter(|offered| offered.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .find_map(|offered| FORMATS.into_iter().find(|&format| format == offered.format))
            .ok_or_else(|| {
                Error::Unsupported("window surface offers no 8-bit UNORM format".into())
            })?;
        let readable = capabilities.supported_usage_flags.contains(READABLE_USAGE);
        let usage = match readable {
            true => vk::ImageUsageFlags::COLOR_ATTACHMENT | READABLE_USAGE,
            false => vk::ImageUsageFlags::COLOR_ATTACHMENT,
        };
        let most = match capabilities.max_image_count {
            0 => u32::MAX,
            most => most,
        };
        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.handle())
            .min_image_count(config.images.clamp(capabilities.min_image_count, most))
            .image_format(format)
            .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .image_extent(config.extent)
            .image_array_layers(1)
            .image_usage(usage)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(composite_alpha(capabilities.supported_composite_alpha))
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(false)
            .old_swapchain(
                old.as_ref()
                    .map_or(vk::SwapchainKHR::null(), |old| old.handle),
            );
        let spare = semaphore(&device)?;
        let mut swapchain = Swapchain {
            device: device.clone(),
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            handle: vk::SwapchainKHR::null(),
            config,
            format,
            readable,
            images: Vec::new(),
            spare,
        };
        swapchain.handle = unsafe { win32.swapchain.create_swapchain(&info, None)? };
        drop(old);
        let handle = swapchain.handle;
        for image in unsafe { win32.swapchain.get_swapchain_images(handle)? } {
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format)
                .subresource_range(color_subresource_range());
            let view = unsafe { device.handle().create_image_view(&view_info, None)? };
            device.set_object_name(view, "rikka.swapchain.image");
            swapchain.images.push(SwapchainImage {
                image,
                view,
                access: ImageAccess::UNDEFINED,
                acquired: vk::Semaphore::null(),
                rendered: vk::Semaphore::null(),
            });
            let last = swapchain.images.len() - 1;
            swapchain.images[last].acquired = semaphore(&device)?;
            swapchain.images[last].rendered = semaphore(&device)?;
        }
        Ok(swapchain)
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn config(&self) -> SwapchainConfig {
        self.config
    }

    pub fn extent(&self) -> vk::Extent2D {
        self.config.extent
    }

    pub fn format(&self) -> vk::Format {
        self.format
    }

    pub fn is_readable(&self) -> bool {
        self.readable
    }

    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    pub fn image(&self, index: u32) -> &SwapchainImage {
        &self.images[index as usize]
    }

    pub fn image_mut(&mut self, index: u32) -> &mut SwapchainImage {
        &mut self.images[index as usize]
    }

    pub fn acquire(&mut self, timeout: Duration) -> Result<Acquire> {
        let win32 = presenting(&self.device)?;
        let nanos = u64::try_from(timeout.as_nanos()).unwrap_or(u64::MAX);
        let acquired = unsafe {
            win32
                .swapchain
                .acquire_next_image(self.handle, nanos, self.spare, vk::Fence::null())
        };
        match acquired {
            Ok((index, suboptimal)) => {
                let image = &mut self.images[index as usize];
                std::mem::swap(&mut self.spare, &mut image.acquired);
                Ok(Acquire::Image { index, suboptimal })
            }
            Err(vk::Result::TIMEOUT | vk::Result::NOT_READY) => Ok(Acquire::Timeout),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(Acquire::Outdated),
            Err(error) => {
                self.device.observe_loss(error);
                Err(error.into())
            }
        }
    }

    pub fn present(&mut self, index: u32, damage: &[vk::RectLayerKHR]) -> Result<Shown> {
        let win32 = presenting(&self.device)?;
        let waits = [self.images[index as usize].rendered];
        let swapchains = [self.handle];
        let indices = [index];
        let regions = [vk::PresentRegionKHR::default().rectangles(damage)];
        let mut incremental = vk::PresentRegionsKHR::default().regions(&regions);
        let mut info = vk::PresentInfoKHR::default()
            .wait_semaphores(&waits)
            .swapchains(&swapchains)
            .image_indices(&indices);
        if win32.incremental_present && !damage.is_empty() {
            info = info.push_next(&mut incremental);
        }
        let queue = self.device.graphics_queue();
        let presented = {
            let _guard = queue.guard();
            unsafe { win32.swapchain.queue_present(queue.handle(), &info) }
        };
        match presented {
            Ok(false) => Ok(Shown::Current),
            Ok(true) => Ok(Shown::Suboptimal),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(Shown::Outdated),
            Err(error) => {
                self.device.observe_loss(error);
                Err(error.into())
            }
        }
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        let device = self.device.handle();
        let queue = self.device.graphics_queue();
        let idle = {
            let _guard = queue.guard();
            unsafe { device.queue_wait_idle(queue.handle()) }
        };
        if let Err(error) = idle {
            tracing::warn!("failed to wait for the swapchain's frames: {error}");
        }
        unsafe {
            for image in &self.images {
                device.destroy_image_view(image.view, None);
                device.destroy_semaphore(image.acquired, None);
                device.destroy_semaphore(image.rendered, None);
            }
            device.destroy_semaphore(self.spare, None);
            if let Some(win32) = self.device.win32() {
                win32.swapchain.destroy_swapchain(self.handle, None);
            }
        }
    }
}

impl std::fmt::Debug for Swapchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Swapchain")
            .field("config", &self.config)
            .field("format", &self.format)
            .field("images", &self.images.len())
            .finish_non_exhaustive()
    }
}

fn semaphore(device: &RenderDevice) -> Result<vk::Semaphore> {
    let semaphore = unsafe {
        device
            .handle()
            .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?
    };
    device.set_object_name(semaphore, "rikka.swapchain.semaphore");
    Ok(semaphore)
}

fn composite_alpha(supported: vk::CompositeAlphaFlagsKHR) -> vk::CompositeAlphaFlagsKHR {
    [
        vk::CompositeAlphaFlagsKHR::OPAQUE,
        vk::CompositeAlphaFlagsKHR::INHERIT,
    ]
    .into_iter()
    .find(|&mode| supported.contains(mode))
    .unwrap_or(vk::CompositeAlphaFlagsKHR::from_raw(
        supported.as_raw() & supported.as_raw().wrapping_neg(),
    ))
}
