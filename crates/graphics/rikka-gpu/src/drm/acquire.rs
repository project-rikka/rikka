use std::collections::VecDeque;
use std::os::fd::{AsRawFd, IntoRawFd, OwnedFd};
use std::sync::Arc;

use ash::{khr, vk};

use crate::core::device::RenderDevice;
use crate::error::Result;

pub(crate) struct AcquireFences {
    device: Arc<RenderDevice>,
    loader: khr::external_semaphore_fd::Device,
    free: Vec<vk::Semaphore>,
    lent: Vec<vk::Semaphore>,
    spent: VecDeque<(u64, Vec<vk::Semaphore>)>,
}

impl AcquireFences {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Option<Self> {
        let loader = device
            .dmabuf()
            .filter(|dmabuf| dmabuf.support.sync_file_import)
            .and_then(|dmabuf| dmabuf.semaphore_fd.clone())?;
        Some(Self {
            device,
            loader,
            free: Vec::new(),
            lent: Vec::new(),
            spent: VecDeque::new(),
        })
    }

    pub(crate) fn import_fence(&mut self, fence: OwnedFd) -> Result<vk::Semaphore> {
        let semaphore = self.holding(fence)?;
        self.lent.push(semaphore);
        Ok(semaphore)
    }

    fn holding(&mut self, fence: OwnedFd) -> Result<vk::Semaphore> {
        let semaphore = match self.free.pop() {
            Some(semaphore) => semaphore,
            None => {
                let info = vk::SemaphoreCreateInfo::default();
                let semaphore = unsafe { self.device.handle().create_semaphore(&info, None)? };
                self.device
                    .set_object_name(semaphore, "rikka.dmabuf.acquire_fence");
                semaphore
            }
        };

        let info = vk::ImportSemaphoreFdInfoKHR::default()
            .semaphore(semaphore)
            .flags(vk::SemaphoreImportFlags::TEMPORARY)
            .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD)
            .fd(fence.as_raw_fd());
        match unsafe { self.loader.import_semaphore_fd(&info) } {
            Ok(()) => {
                let _ = fence.into_raw_fd();
                Ok(semaphore)
            }
            Err(err) => {
                self.free.push(semaphore);
                Err(err.into())
            }
        }
    }

    pub(crate) fn submitted(&mut self, frame_value: u64) {
        if self.lent.is_empty() {
            return;
        }
        self.spent
            .push_back((frame_value, std::mem::take(&mut self.lent)));
    }

    pub(crate) fn collect(&mut self, completed_value: u64) {
        while let Some((value, _)) = self.spent.front() {
            if *value > completed_value {
                break;
            }
            let (_, semaphores) = self.spent.pop_front().expect("entry exists");
            self.free.extend(semaphores);
        }
    }
}

impl std::fmt::Debug for AcquireFences {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcquireFences")
            .field("lent", &self.lent.len())
            .field("free", &self.free.len())
            .finish_non_exhaustive()
    }
}

impl Drop for AcquireFences {
    fn drop(&mut self) {
        unsafe {
            for semaphore in self
                .free
                .drain(..)
                .chain(self.lent.drain(..))
                .chain(self.spent.drain(..).flat_map(|(_, semaphores)| semaphores))
            {
                self.device.handle().destroy_semaphore(semaphore, None);
            }
        }
    }
}
