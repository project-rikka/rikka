use std::time::Instant;

use rikka_present::Presenter;
use rikka_ui::{Event, WindowRef};
use rikka_ui_host::Embedder;
use rikka_wayland_client::wl_surface::WlSurface;

use crate::tasks::WindowId;
use crate::wayland::client::Client;
use crate::wayland::session::Session;
use crate::wayland::window_state::WindowState;

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

pub(crate) fn window_of(hosting: &Hosting, surface: &WlSurface) -> Option<WindowId> {
    hosting
        .iter()
        .find(|(_, hosted)| hosted.state.toplevel.surface() == surface)
        .map(|(id, _)| *id)
}

pub(crate) struct Windows<'a> {
    pub(crate) hosting: &'a mut Hosting,
    pub(crate) session: &'a Session,
    pub(crate) client: &'a Client,
    closing: &'a mut Vec<WindowId>,
}

impl<'a> Windows<'a> {
    pub(crate) fn new(
        hosting: &'a mut Hosting,
        session: &'a Session,
        client: &'a Client,
        closing: &'a mut Vec<WindowId>,
    ) -> Self {
        Self {
            hosting,
            session,
            client,
            closing,
        }
    }

    pub(crate) fn drawing_on(&self, surface: &WlSurface) -> Option<WindowId> {
        self.hosting
            .iter()
            .find(|(_, hosted)| hosted.state.toplevel.surface() == surface)
            .map(|(id, _)| *id)
    }

    pub(crate) fn dispatch_event(&mut self, id: WindowId, event: Event) {
        if let Some(hosted) = self.hosting.get_mut(&id) {
            hosted.ui.handle_event(event);
        }
        self.service(id);
    }

    pub(crate) fn closes(&self, id: WindowId) -> bool {
        self.closing.contains(&id)
    }

    pub(crate) fn close(&mut self, id: WindowId) {
        if !self.closing.contains(&id) {
            self.closing.push(id);
        }
    }

    pub(crate) fn withhold(&mut self, id: WindowId, withheld: bool) {
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        hosted.state.withhold(withheld);
        let drawable = hosted.state.is_drawable();
        if drawable && !hosted.is_drawable() {
            hosted.frame_answered();
        }
        hosted.drawable(drawable);
        if drawable {
            hosted.ui.request_redraw();
        }
    }

    pub(crate) fn service(&mut self, id: WindowId) {
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        let closes = hosted
            .state
            .service(&mut hosted.ui, self.session, self.client);
        let raising = hosted.state.take_raising();
        let quits = hosted.state.take_quitting();
        if closes {
            self.close(id);
        }
        self.raise(id, raising);
        if quits {
            self.quit();
        }
    }

    pub(crate) fn service_all(&mut self) {
        let mut raising = Vec::new();
        let mut quits = false;
        let mut closing = Vec::new();
        for (id, hosted) in self.hosting.iter_mut() {
            if hosted
                .state
                .service(&mut hosted.ui, self.session, self.client)
            {
                closing.push(*id);
            }
            raising.push((*id, hosted.state.take_raising()));
            quits |= hosted.state.take_quitting();
        }
        for id in closing {
            self.close(id);
        }
        for (id, targets) in raising {
            self.raise(id, targets);
        }
        if quits {
            self.quit();
        }
    }

    fn quit(&mut self) {
        let mut closing = Vec::new();
        for (id, hosted) in self.hosting.iter_mut() {
            if self.closing.contains(id) {
                continue;
            }
            let closes = hosted.state.closing.ask(&mut hosted.ui)
                || hosted
                    .state
                    .service(&mut hosted.ui, self.session, self.client);
            if closes {
                closing.push(*id);
            }
        }
        for id in closing {
            self.close(id);
        }
    }

    fn raise(&self, from: WindowId, targets: Vec<WindowRef>) {
        let Some(asking) = self.hosting.get(&from) else {
            return;
        };
        for target in targets {
            match self.hosting.get(&WindowId::from(target.get())) {
                Some(raised) => self.session.transfer_focus(
                    asking.state.toplevel.surface(),
                    raised.state.toplevel.surface(),
                ),
                None => tracing::debug!("no window matches {target:?}"),
            }
        }
    }
}
