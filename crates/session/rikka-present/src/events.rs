use rikka_sync::{Condvar, Mutex};
use std::sync::Arc;
use std::task::Wake;
use std::time::Instant;

use rikka_wayland_client::wl_buffer::{self, WlBuffer};
use rikka_wayland_client::zwp_linux_dmabuf_feedback_v1::{
    self, TrancheFlags, ZwpLinuxDmabufFeedbackV1,
};

use crate::feedback::{Feedback, Gathering};

#[derive(Debug, Default)]
pub(crate) struct Events {
    pub(crate) released: Vec<u32>,
    pub(crate) feedback: Option<Feedback>,
    gathering: Gathering,
}

pub(crate) fn on_wl_buffer(events: &mut Events, buffer: WlBuffer, event: wl_buffer::Event) {
    if let wl_buffer::Event::Release = event
        && let Some(id) = buffer.key::<u32>()
    {
        events.released.push(id);
    }
}

pub(crate) fn on_zwp_linux_dmabuf_feedback_v1(
    events: &mut Events,
    _: ZwpLinuxDmabufFeedbackV1,
    event: zwp_linux_dmabuf_feedback_v1::Event,
) {
    use zwp_linux_dmabuf_feedback_v1::Event;
    let gathering = &mut events.gathering;
    match event {
        Event::FormatTable { fd, size } => gathering.table(&fd, size),
        Event::MainDevice { device } => gathering.main_device(&device),
        Event::TrancheTargetDevice { device } => gathering.tranche_device(&device),
        Event::TrancheFormats { indices } => gathering.tranche_formats(&indices),
        Event::TrancheFlags { flags } => {
            gathering.tranche_scanout(flags.contains(TrancheFlags::SCANOUT));
        }
        Event::TrancheDone => gathering.tranche_done(),
        Event::Done => events.feedback = Some(gathering.done()),
        _ => {}
    }
}

#[derive(Debug, Default)]
pub(crate) struct Signal {
    rung: Mutex<bool>,
    bell: Condvar,
}

impl Signal {
    pub(crate) fn wait_until(&self, deadline: Instant) {
        let mut rung = self.rung.lock();
        while !*rung {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            rung = self.bell.wait_timeout(rung, left).0;
        }
        *rung = false;
    }
}

impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        *self.rung.lock() = true;
        self.bell.notify_all();
    }
}
