use std::time::Instant;

use rikka_swapchain::Presenter;
use rikka_ui_host::Embedder;

use crate::tasks::WindowId;
use crate::win32::window::state::WindowState;

pub(crate) type Hosting = Embedder<WindowId, WindowState, Presenter>;

pub(crate) fn next_wake(hosting: &Hosting) -> Option<Instant> {
    hosting
        .iter()
        .filter(|(_, hosted)| {
            hosted.has_surface() && !hosted.awaits_frame() && hosted.state.is_drawable()
        })
        .filter_map(|(_, hosted)| hosted.ui.next_wake())
        .min()
}

pub(crate) struct Windows<'a> {
    pub(crate) hosting: &'a mut Hosting,
    touched: &'a mut Vec<WindowId>,
}

impl<'a> Windows<'a> {
    pub(crate) fn new(hosting: &'a mut Hosting, touched: &'a mut Vec<WindowId>) -> Self {
        Self { hosting, touched }
    }

    pub(crate) fn touch(&mut self, id: WindowId) {
        if !self.touched.contains(&id) {
            self.touched.push(id);
        }
    }

    pub(crate) fn dispatch(&mut self, id: WindowId, event: rikka_ui::Event) {
        if let Some(hosted) = self.hosting.get_mut(&id) {
            hosted.ui.handle_event(event);
        }
        self.touch(id);
    }

    pub(crate) fn withhold(&mut self, id: WindowId, withheld: bool) {
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        hosted.state.withhold(withheld);
        let drawable = hosted.state.is_drawable() && hosted.has_surface();
        if drawable && !hosted.is_drawable() {
            hosted.frame_answered();
        }
        hosted.drawable(drawable);
        if drawable {
            hosted.ui.request_redraw();
        }
        self.touch(id);
    }
}
