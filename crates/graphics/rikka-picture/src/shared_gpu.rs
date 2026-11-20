use rikka_sync::{Condvar, Mutex, MutexGuard};
#[cfg(unix)]
use std::sync::Arc;
use std::time::Duration;

use rikka_gpu::Compute;
#[cfg(unix)]
use rikka_gpu::{DrmFormat, Gpu};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedGpu {
    #[cfg(unix)]
    pub primary_node: Option<(u32, u32)>,
    #[cfg(unix)]
    pub render_node: Option<(u32, u32)>,
    #[cfg(unix)]
    pub texture_formats: Vec<DrmFormat>,
}

type Waiter<T> = Box<dyn FnOnce(T) + Send>;

struct Slot<T> {
    value: Option<T>,
    waiters: Vec<Waiter<T>>,
}

struct SetOnce<T> {
    slot: Mutex<Slot<T>>,
    ready: Condvar,
}

impl<T: Clone> SetOnce<T> {
    const fn new() -> SetOnce<T> {
        SetOnce {
            slot: Mutex::new(Slot {
                value: None,
                waiters: Vec::new(),
            }),
            ready: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Slot<T>> {
        self.slot.lock()
    }

    fn get(&self) -> Option<T> {
        self.lock().value.clone()
    }

    fn set(&self, value: T) {
        let mut slot = self.lock();
        slot.value = Some(value.clone());
        let waiters = std::mem::take(&mut slot.waiters);
        drop(slot);
        self.ready.notify_all();
        for waiter in waiters {
            waiter(value.clone());
        }
    }

    fn wait(&self, timeout: Duration) -> Option<T> {
        let (slot, _) = self
            .ready
            .wait_timeout_while(self.lock(), timeout, |slot| slot.value.is_none());
        slot.value.clone()
    }

    fn on_set(&self, waiter: impl FnOnce(T) + Send + 'static) {
        let mut slot = self.lock();
        let Some(value) = slot.value.clone() else {
            slot.waiters.push(Box::new(waiter));
            return;
        };
        drop(slot);
        waiter(value);
    }
}

#[cfg(unix)]
static GPU: SetOnce<Arc<SharedGpu>> = SetOnce::new();

static COMPUTE: SetOnce<Compute> = SetOnce::new();

impl SharedGpu {
    #[cfg(unix)]
    #[must_use]
    pub fn from_gpu(gpu: &Gpu) -> SharedGpu {
        let nodes = gpu.drm_nodes();
        SharedGpu {
            primary_node: nodes.and_then(|nodes| nodes.primary),
            render_node: nodes.and_then(|nodes| nodes.render),
            texture_formats: gpu.dmabuf_texture_formats().to_vec(),
        }
    }

    #[cfg(unix)]
    pub fn publish(self) {
        if GPU.get().as_deref() != Some(&self) {
            GPU.set(Arc::new(self));
        }
    }

    pub fn publish_compute(compute: Compute) {
        if COMPUTE.get().is_none() {
            COMPUTE.set(compute);
        }
    }

    #[cfg(unix)]
    #[must_use]
    pub fn wait(timeout: Duration) -> Option<Arc<SharedGpu>> {
        GPU.wait(timeout)
    }

    #[must_use]
    pub fn compute(timeout: Duration) -> Option<Compute> {
        COMPUTE.wait(timeout)
    }

    pub fn on_compute(notify: impl FnOnce(Compute) + Send + 'static) {
        COMPUTE.on_set(notify);
    }
}
