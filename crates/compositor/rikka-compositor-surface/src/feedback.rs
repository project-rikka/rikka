use std::time::Duration;

use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::wp_presentation_feedback::{Kind, WpPresentationFeedback};
use rikka_wayland_wire::ClientId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refresh {
    Fixed(Duration),
    Variable { fastest: Duration },
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Presentation {
    pub time: Duration,
    pub refresh: Refresh,
    pub sequence: u64,
    pub flags: Kind,
}

#[must_use = "dropping feedback discards it"]
#[derive(Debug, Default)]
pub struct Feedback {
    waiting: Vec<WpPresentationFeedback>,
}

impl Feedback {
    pub(crate) fn take_from(&mut self, waiting: &mut Vec<WpPresentationFeedback>) {
        self.waiting.append(waiting);
    }

    pub fn absorb(&mut self, mut other: Feedback) {
        self.waiting.append(&mut other.waiting);
    }

    pub fn presented<I>(mut self, presentation: &Presentation, outputs: impl Fn(ClientId) -> I)
    where
        I: IntoIterator<Item = WlOutput>,
    {
        let seconds = presentation.time.as_secs();
        for feedback in std::mem::take(&mut self.waiting) {
            for output in outputs(feedback.client()) {
                feedback.sync_output(&output);
            }
            let refresh = match presentation.refresh {
                Refresh::Fixed(period) => nanoseconds(period),
                Refresh::Variable { fastest } if feedback.version() >= 2 => nanoseconds(fastest),
                Refresh::Variable { .. } | Refresh::Unknown => 0,
            };
            feedback.presented(
                (seconds >> 32) as u32,
                seconds as u32,
                presentation.time.subsec_nanos(),
                refresh,
                (presentation.sequence >> 32) as u32,
                presentation.sequence as u32,
                presentation.flags,
            );
        }
    }

    pub fn discarded(self) {}
}

impl Drop for Feedback {
    fn drop(&mut self) {
        for feedback in self.waiting.drain(..) {
            feedback.discarded();
        }
    }
}

fn nanoseconds(period: Duration) -> u32 {
    u32::try_from(period.as_nanos()).unwrap_or(u32::MAX)
}
