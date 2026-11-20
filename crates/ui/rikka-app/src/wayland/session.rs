use std::sync::Arc;

use rikka_blur::{BlurredSurface, Glass, OpaqueRegions, SurfaceRegions};
use rikka_clipboard::Clipboard;
use rikka_seam::net::SocketClient;
use rikka_session::toplevels::{State, ToplevelsClient};
use rikka_wayland::{Host, Presentation};
use rikka_wayland_client::wl_surface::WlSurface;

use crate::tasks::{LoopEvent, LoopProxy, WindowId};
use crate::wayland::activation::Raise;
use crate::wayland::data_device::{DragEvent, Drags};
use crate::wayland::feedback::FeedbackTracker;
use crate::wayland::inhibit::{ShortcutInhibitor, Shortcuts};

#[derive(Default)]
pub(crate) struct Session {
    host: Option<Host>,
    glass: Option<Glass>,
    opaque_regions: Option<OpaqueRegions>,
    shortcuts: Option<Shortcuts>,
    raise: Option<Raise>,
    drags: Option<Drags>,
    presentation: Option<Arc<Presentation>>,
    app_id: String,
}

impl Session {
    pub(crate) fn open(host: &Host, proxy: &LoopProxy, app_id: &str) -> Session {
        open_clipboard(host);
        Session {
            glass: Glass::bind(host),
            opaque_regions: OpaqueRegions::bind(host),
            shortcuts: Shortcuts::on(host),
            raise: Raise::on(host),
            drags: drags(host, proxy.clone()),
            presentation: presentation(host, proxy.clone()),
            host: Some(host.clone()),
            app_id: app_id.to_owned(),
        }
    }

    pub(crate) fn drags(&self) -> Option<&Drags> {
        self.drags.as_ref()
    }

    pub(crate) fn glaze(&self, surface: &WlSurface, blurs: bool, opaque: bool) -> SurfaceRegions {
        let opaque_region = opaque
            .then(|| Some(self.opaque_regions.as_ref()?.for_surface(surface.clone())))
            .flatten();
        SurfaceRegions::new(self.blurred(surface), blurs).with_opaque_region(opaque_region)
    }

    pub(crate) fn blurred(&self, surface: &WlSurface) -> Option<BlurredSurface> {
        self.glass.as_ref().map(|glass| glass.for_surface(surface))
    }

    pub(crate) fn feedback(&self, surface: &WlSurface, id: WindowId) -> Option<FeedbackTracker> {
        let presentation = Arc::clone(self.presentation.as_ref()?);
        Some(FeedbackTracker::new(presentation, surface.clone(), id))
    }

    pub(crate) fn inhibit_shortcuts(&self, surface: &WlSurface) -> Option<ShortcutInhibitor> {
        Some(self.shortcuts.as_ref()?.inhibit(surface))
    }

    pub(crate) fn transfer_focus(&self, from: &WlSurface, to: &WlSurface) {
        match self.raise.as_ref() {
            Some(raise) => raise.transfer_focus(from, to, &self.app_id),
            None => tracing::debug!("activation unsupported by session"),
        }
    }

    pub(crate) fn activate(&self, surface: &WlSurface, token: &str) {
        match self.raise.as_ref() {
            Some(raise) => raise.forward(surface, token),
            None => tracing::debug!("activation tokens unsupported by session"),
        }
    }

    pub(crate) fn reactivate(
        &self,
        surface: Option<&WlSurface>,
        token: Option<&str>,
        app_id: &str,
    ) {
        if let (Some(raise), Some(surface), Some(token)) = (self.raise.as_ref(), surface, token) {
            raise.forward(surface, token);
            return;
        }
        focus_existing_window(app_id.to_owned());
    }

    pub(crate) fn release(&mut self) {
        self.presentation = None;
        self.drags = None;
        rikka_ui::clipboard::close_session();
        self.glass = None;
        self.opaque_regions = None;
        self.shortcuts = None;
        self.raise = None;
        self.host = None;
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("open", &self.host.is_some())
            .finish_non_exhaustive()
    }
}

fn drags(host: &Host, proxy: LoopProxy) -> Option<Drags> {
    let sink = move |event: DragEvent| {
        let _ = proxy.send_event(LoopEvent::Drag(Box::new(event)));
    };
    match Drags::on(host, sink) {
        Ok(drags) => Some(drags),
        Err(error) => {
            tracing::warn!("drag and drop unavailable: {error}");
            None
        }
    }
}

fn presentation(host: &Host, proxy: LoopProxy) -> Option<Arc<Presentation>> {
    let sink = move |feedback| {
        let _ = proxy.send_event(LoopEvent::Presented(feedback));
    };
    Presentation::new(host.display(), sink)
        .inspect_err(|error| {
            tracing::debug!(%error, "presentation-time unavailable, pacing without feedback")
        })
        .ok()
        .map(Arc::new)
}

fn open_clipboard(host: &Host) {
    match Clipboard::for_host(host) {
        Ok(clipboard) => rikka_ui::clipboard::install_session(clipboard),
        Err(error) => tracing::warn!("failed to open session clipboard: {error}"),
    }
}

fn focus_existing_window(app_id: String) {
    let spawned = std::thread::Builder::new()
        .name("rikka-app-raise".to_owned())
        .spawn(move || {
            if let Err(error) = request_focus(&app_id) {
                tracing::debug!(%error, app_id, "failed to focus existing window");
            }
        });
    if let Err(error) = spawned {
        tracing::warn!("failed to request window activation: {error}");
    }
}

fn request_focus(app_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let connection = SocketClient::from_env()?;
    let toplevels = ToplevelsClient::new(connection.client());
    let state = toplevels.state().blocking_wait()?.decode::<State>()?;
    let Some(window) = state
        .toplevels
        .iter()
        .find(|toplevel| toplevel.app_id == app_id)
    else {
        tracing::debug!("no window of `{app_id}` to activate");
        return Ok(());
    };
    toplevels.focus(&window.id).blocking_wait()?;
    Ok(())
}
