use std::sync::Arc;

use crate::core::background::{Pool, Slot};
use crate::core::device::RenderDevice;
use crate::error::Result;

static PROVISION: Pool = Pool::new("rikka-provision", || 1);

pub struct Provision<T> {
    slot: Arc<Slot<Option<Result<T>>>>,
}

impl<T: Send + 'static> Provision<T> {
    pub fn start(
        device: &Arc<RenderDevice>,
        make: impl FnOnce(Arc<RenderDevice>) -> Result<T> + Send + 'static,
    ) -> Self {
        let slot = Arc::new(Slot::queued());
        let serving = Arc::clone(&slot);
        let device = Arc::downgrade(device);
        PROVISION.enqueue(move || {
            serving.run(|| device.upgrade().map(make));
        });
        Self { slot }
    }

    pub fn take(&self) -> Option<Result<T>> {
        self.slot.take().flatten()
    }
}

impl<T> Drop for Provision<T> {
    fn drop(&mut self) {
        drop(self.slot.claim());
    }
}

impl<T> std::fmt::Debug for Provision<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Provision").finish_non_exhaustive()
    }
}
