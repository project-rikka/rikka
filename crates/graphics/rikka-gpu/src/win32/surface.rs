use std::sync::Arc;

use ash::vk;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use crate::core::device::RenderDevice;
use crate::error::{Error, Result};
use crate::win32::Win32Device;

pub(crate) const READABLE_USAGE: vk::ImageUsageFlags = vk::ImageUsageFlags::from_raw(
    vk::ImageUsageFlags::SAMPLED.as_raw() | vk::ImageUsageFlags::TRANSFER_SRC.as_raw(),
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceFit {
    pub extent: vk::Extent2D,
    pub readable: bool,
}

pub struct Surface {
    device: Arc<RenderDevice>,
    handle: vk::SurfaceKHR,
}

impl Surface {
    pub fn new(device: Arc<RenderDevice>, window: isize) -> Result<Surface> {
        let win32 = presenting(&device)?;
        let module = unsafe { GetModuleHandleW(None) }
            .map_err(|error| Error::Unsupported(format!("no module handle: {error}")))?;
        let info = vk::Win32SurfaceCreateInfoKHR::default()
            .hinstance(module.0 as vk::HINSTANCE)
            .hwnd(window as vk::HWND);
        let handle = unsafe { win32.win32_surface.create_win32_surface(&info, None)? };
        let surface = Surface {
            device: device.clone(),
            handle,
        };
        let supported = unsafe {
            win32.surface.get_physical_device_surface_support(
                device.physical(),
                device.queue_family(),
                handle,
            )?
        };
        if !supported {
            return Err(Error::Unsupported(
                "the graphics queue cannot present to this window".into(),
            ));
        }
        Ok(surface)
    }

    pub fn fit(&self, requested: vk::Extent2D) -> Result<Option<SurfaceFit>> {
        let capabilities = self.capabilities()?;
        let extent = match capabilities.current_extent.width {
            u32::MAX => vk::Extent2D {
                width: requested.width.clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ),
                height: requested.height.clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ),
            },
            _ => capabilities.current_extent,
        };
        Ok((extent.width > 0 && extent.height > 0).then(|| SurfaceFit {
            extent,
            readable: capabilities.supported_usage_flags.contains(READABLE_USAGE),
        }))
    }

    pub fn device(&self) -> &Arc<RenderDevice> {
        &self.device
    }

    pub(crate) fn handle(&self) -> vk::SurfaceKHR {
        self.handle
    }

    pub(crate) fn capabilities(&self) -> Result<vk::SurfaceCapabilitiesKHR> {
        let win32 = presenting(&self.device)?;
        Ok(unsafe {
            win32
                .surface
                .get_physical_device_surface_capabilities(self.device.physical(), self.handle)?
        })
    }

    pub(crate) fn formats(&self) -> Result<Vec<vk::SurfaceFormatKHR>> {
        let win32 = presenting(&self.device)?;
        Ok(unsafe {
            win32
                .surface
                .get_physical_device_surface_formats(self.device.physical(), self.handle)?
        })
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if let Some(win32) = self.device.win32() {
            unsafe { win32.surface.destroy_surface(self.handle, None) };
        }
    }
}

impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Surface").finish_non_exhaustive()
    }
}

pub(crate) fn presenting(device: &RenderDevice) -> Result<&Win32Device> {
    device
        .win32()
        .ok_or_else(|| Error::Unsupported("device cannot present to windows".into()))
}
