use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ash::vk;
use rikka_sync::Mutex;

use crate::compute::context::Shared;
use crate::compute::lane::Lane;
use crate::core::device::RenderDevice;
use crate::core::ticks::Ticks;
use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub name: &'static str,
    pub milliseconds: f64,
}

pub(crate) struct Clock {
    device: Arc<RenderDevice>,
    pool: Mutex<vk::QueryPool>,
    names: Mutex<Vec<&'static str>>,
    ticks: Option<Ticks>,
    ready: AtomicU64,
}

impl Clock {
    pub(crate) fn new(device: Arc<RenderDevice>, family: u32) -> Arc<Clock> {
        Arc::new(Clock {
            ticks: Ticks::new(&device, family),
            device,
            pool: Mutex::new(vk::QueryPool::null()),
            names: Mutex::new(Vec::new()),
            ready: AtomicU64::new(0),
        })
    }

    pub(crate) fn ticks(&self) -> bool {
        self.ticks.is_some()
    }

    pub(crate) fn prepare(
        &self,
        cmd: vk::CommandBuffer,
        names: Vec<&'static str>,
    ) -> Result<Option<vk::QueryPool>> {
        if !self.ticks() || names.is_empty() {
            return Ok(None);
        }
        let count = names.len() as u32 + 1;
        let info = vk::QueryPoolCreateInfo::default()
            .query_type(vk::QueryType::TIMESTAMP)
            .query_count(count);
        let pool = unsafe { self.device.handle().create_query_pool(&info, None)? };
        self.device.set_object_name(pool, "rikka.compute.timing");
        unsafe {
            self.device
                .handle()
                .cmd_reset_query_pool(cmd, pool, 0, count)
        };
        *self.pool.lock() = pool;
        *self.names.lock() = names;
        Ok(Some(pool))
    }

    pub(crate) fn complete_at(&self, value: u64) {
        self.ready.store(value, Ordering::Release);
    }

    fn spans(&self) -> Result<Vec<Span>> {
        let pool = *self.pool.lock();
        let names = self.names.lock();
        let Some(ticks) = self.ticks.filter(|_| pool != vk::QueryPool::null()) else {
            return Ok(Vec::new());
        };
        let mut stamps = vec![0u64; names.len() + 1];
        unsafe {
            self.device.handle().get_query_pool_results(
                pool,
                0,
                &mut stamps,
                vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT,
            )?
        };
        Ok(names
            .iter()
            .zip(stamps.windows(2))
            .map(|(name, pair)| Span {
                name,
                milliseconds: ticks.nanos([pair[0], pair[1]]) / 1.0e6,
            })
            .collect())
    }
}

impl Drop for Clock {
    fn drop(&mut self) {
        let pool = *self.pool.get_mut();
        if pool != vk::QueryPool::null() {
            unsafe { self.device.handle().destroy_query_pool(pool, None) };
        }
    }
}

#[must_use = "a timing is only read by waiting on it"]
pub struct Timing {
    pub(crate) shared: Arc<Shared>,
    pub(crate) lane: Lane,
    pub(crate) clock: Arc<Clock>,
}

impl Timing {
    pub fn wait(self) -> Result<Vec<Span>> {
        let ready = self.clock.ready.load(Ordering::Acquire);
        if ready == 0 {
            return Err(Error::Invalid(
                "timing read before its batch was submitted".into(),
            ));
        }
        self.shared.line(self.lane).timeline.wait(ready)?;
        let spans = self.clock.spans();
        self.shared.collect();
        spans
    }
}

impl std::fmt::Debug for Timing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timing").finish_non_exhaustive()
    }
}
