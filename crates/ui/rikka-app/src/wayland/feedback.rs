use std::sync::Arc;

use rikka_wayland::Presentation;
use rikka_wayland_client::wl_surface::WlSurface;

use crate::tasks::WindowId;

#[derive(Debug)]
pub(crate) struct FeedbackTracker {
    presentation: Arc<Presentation>,
    surface: WlSurface,
    key: u64,
}

impl FeedbackTracker {
    pub(crate) fn new(
        presentation: Arc<Presentation>,
        surface: WlSurface,
        window: WindowId,
    ) -> FeedbackTracker {
        FeedbackTracker {
            presentation,
            surface,
            key: u64::from(window),
        }
    }

    pub(crate) fn watch_next_commit(&self) {
        self.presentation.feedback(&self.surface, self.key);
    }
}
