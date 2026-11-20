use std::sync::Arc;
use std::thread::JoinHandle;

use ash::vk;
use rikka_sync::Mutex;

use crate::compute::lane::{Lane, Marks};
use crate::core::device::RenderDevice;
use crate::core::frame::FrameTimeline;
use crate::error::Result;

type Notify = Box<dyn FnOnce() + Send>;

pub(crate) struct Completions {
    watch: Arc<Watch>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

struct Watch {
    device: Arc<RenderDevice>,
    timelines: [Arc<FrameTimeline>; 2],
    wake: Arc<FrameTimeline>,
    pending: Mutex<Pending>,
}

#[derive(Default)]
struct Pending {
    waiters: Vec<Waiter>,
    wakes: u64,
    closed: bool,
}

struct Waiter {
    lane: Lane,
    value: u64,
    notify: Notify,
}

impl Completions {
    pub(crate) fn new(
        device: &Arc<RenderDevice>,
        timelines: [Arc<FrameTimeline>; 2],
    ) -> Result<Completions> {
        Ok(Completions {
            watch: Arc::new(Watch {
                device: Arc::clone(device),
                timelines,
                wake: FrameTimeline::named(Arc::clone(device), "rikka.compute.completion_wake")?,
                pending: Mutex::new(Pending::default()),
            }),
            thread: Mutex::new(None),
        })
    }

    pub(crate) fn notify(&self, lane: Lane, value: u64, notify: Notify) {
        if self.watch.timelines[lane.index()].reached(value) {
            notify();
            return;
        }
        if let Err(error) = self.ensure_thread() {
            tracing::error!(%error, "cannot start compute completion thread");
            notify();
            return;
        }
        let mut pending = self.watch.pending.lock();
        if pending.closed {
            drop(pending);
            notify();
            return;
        }
        pending.waiters.push(Waiter {
            lane,
            value,
            notify,
        });
        self.watch.wake_thread(&mut pending);
    }

    fn ensure_thread(&self) -> std::io::Result<()> {
        let mut thread = self.thread.lock();
        if thread.is_none() {
            let watch = Arc::clone(&self.watch);
            *thread = Some(
                std::thread::Builder::new()
                    .name("rikka-compute-completion".to_owned())
                    .spawn(move || watch.run())?,
            );
        }
        Ok(())
    }
}

impl Drop for Completions {
    fn drop(&mut self) {
        let Some(thread) = self.thread.get_mut().take() else {
            return;
        };
        {
            let mut pending = self.watch.pending.lock();
            pending.closed = true;
            self.watch.wake_thread(&mut pending);
        }
        if thread.thread().id() == std::thread::current().id() {
            return;
        }
        if thread.join().is_err() {
            tracing::error!("compute completion thread panicked");
        }
    }
}

impl Watch {
    fn wake_thread(&self, pending: &mut Pending) {
        pending.wakes += 1;
        if let Err(error) = self.wake.signal(pending.wakes) {
            tracing::error!(%error, "cannot wake compute completion thread");
        }
    }

    fn run(&self) {
        loop {
            let (targets, wakes, closed) = {
                let pending = self.pending.lock();
                (earliest(&pending.waiters), pending.wakes, pending.closed)
            };
            if closed {
                self.fire(|_| true);
                return;
            }
            if let Err(error) = self.wait_any(targets, wakes + 1) {
                tracing::error!(%error, "cannot wait for compute work; releasing every waiter");
                self.pending.lock().closed = true;
                self.fire(|_| true);
                return;
            }
            let finished = [self.timelines[0].completed(), self.timelines[1].completed()];
            self.fire(|waiter| waiter.value <= finished[waiter.lane.index()]);
        }
    }

    fn wait_any(&self, targets: Marks, wake: u64) -> Result<()> {
        let mut semaphores = vec![self.wake.handle()];
        let mut values = vec![wake];
        for lane in Lane::ALL {
            let target = targets[lane.index()];
            if target > 0 {
                semaphores.push(self.timelines[lane.index()].handle());
                values.push(target);
            }
        }
        let info = vk::SemaphoreWaitInfo::default()
            .flags(vk::SemaphoreWaitFlags::ANY)
            .semaphores(&semaphores)
            .values(&values);
        unsafe { self.device.handle().wait_semaphores(&info, u64::MAX) }
            .inspect_err(|result| self.device.observe_loss(*result))?;
        Ok(())
    }

    fn fire(&self, done: impl Fn(&Waiter) -> bool) {
        let ready: Vec<Waiter> = {
            let mut pending = self.pending.lock();
            let (ready, waiting) = std::mem::take(&mut pending.waiters)
                .into_iter()
                .partition(done);
            pending.waiters = waiting;
            ready
        };
        for waiter in ready {
            (waiter.notify)();
        }
    }
}

fn earliest(waiters: &[Waiter]) -> Marks {
    let mut targets = [0; 2];
    for waiter in waiters {
        let target = &mut targets[waiter.lane.index()];
        *target = match *target {
            0 => waiter.value,
            current => current.min(waiter.value),
        };
    }
    targets
}
