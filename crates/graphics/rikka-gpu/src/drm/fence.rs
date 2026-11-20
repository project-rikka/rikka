use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::Arc;

use ash::{khr, vk};

use crate::core::device::RenderDevice;
use crate::error::{Error, Result};

pub struct ExportSemaphore {
    device: Arc<RenderDevice>,
    loader: khr::external_semaphore_fd::Device,
    semaphore: vk::Semaphore,
}

impl ExportSemaphore {
    pub fn new(device: Arc<RenderDevice>) -> Result<Option<Self>> {
        let Some(loader) = device
            .dmabuf()
            .filter(|dmabuf| dmabuf.support.sync_file_export)
            .and_then(|dmabuf| dmabuf.semaphore_fd.clone())
        else {
            return Ok(None);
        };

        let mut export = vk::ExportSemaphoreCreateInfo::default()
            .handle_types(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
        let info = vk::SemaphoreCreateInfo::default().push_next(&mut export);
        let semaphore = unsafe { device.handle().create_semaphore(&info, None)? };
        device.set_object_name(semaphore, "rikka.dmabuf.release_fence");
        Ok(Some(Self {
            device,
            loader,
            semaphore,
        }))
    }

    pub fn handle(&self) -> vk::Semaphore {
        self.semaphore
    }

    pub fn export(&self) -> Result<Option<OwnedFd>> {
        let info = vk::SemaphoreGetFdInfoKHR::default()
            .semaphore(self.semaphore)
            .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
        let fd = unsafe { self.loader.get_semaphore_fd(&info)? };
        if fd < 0 {
            return Ok(None);
        }
        Ok(Some(unsafe { OwnedFd::from_raw_fd(fd) }))
    }
}

impl std::fmt::Debug for ExportSemaphore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportSemaphore").finish_non_exhaustive()
    }
}

impl Drop for ExportSemaphore {
    fn drop(&mut self) {
        unsafe { self.device.handle().destroy_semaphore(self.semaphore, None) };
    }
}

pub(crate) fn wait_signalled(fence: &OwnedFd) -> Result<()> {
    loop {
        let mut watched = libc::pollfd {
            fd: fence.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&raw mut watched, 1, -1) } > 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(Error::Driver(Box::new(error)));
        }
    }
}
