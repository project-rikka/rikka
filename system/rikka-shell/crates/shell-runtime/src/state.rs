use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use rikka_blur::Glass;
use rikka_seat::{SeatEvent, SeatHost, Seats};
use rikka_ui_host::{Embedder, WakeHint};
use rikka_ui_video::VideoStore;
use rikka_wayland::Display;
use rikka_wayland_client::wl_output::WlOutput;
use rikka_wayland_client::wl_registry::WlRegistry;
use rikka_wayland_client::wl_seat::WlSeat;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::{ObjectId, Proxy, Route};
use rikka_window_view::WindowViews;

use super::bridge::{Bridge, BridgeParts};
use super::idling::Idling;
use super::locking::Locker;
use super::screens::{self, Screens};
use super::senders::{TaskMessage, TaskSender};
use super::surfaces::{Surfaces, UnmapSync, Wayland};
use super::timers::{Alarm, Timers};
use crate::error::{Error, Result};
use shell_kit::Host;
use shell_kit::KeyboardState;
use shell_kit::hosting::Parts;

const COMPOSITOR: u32 = 6;
const LAYER_SHELL: u32 = 4;
const XDG_OUTPUTS: u32 = 3;
const OUTPUT: u32 = 4;
const SESSION_LOCK: u32 = 1;
const IDLE_NOTIFIER: u32 = 2;
const ALPHA_MODIFIER: u32 = 1;

pub(super) struct ShellState {
    pub(super) screens: Screens,
    pub(super) seats: Seats,
    pub(super) surfaces: Surfaces,
    pub(super) idling: Idling,
    pub(super) host: Host,
    keyboard: KeyboardState,
    bridge: Bridge,
    tasks: Receiver<TaskMessage>,
    timers: Timers,
    polling: Option<Instant>,
    stopped: bool,
    error: Option<Error>,
}

impl ShellState {
    pub(crate) fn new(shell: Parts, display: Display, route: Route) -> Result<Self> {
        let (task_letters, tasks) = shell.waker.channel::<TaskMessage>();
        let mut embedder = Embedder::new(TaskSender::new(task_letters));
        let waker = shell.waker.clone();
        embedder.wakes_with(Arc::new(move || waker.ring()));
        embedder.plays_video(Box::new(VideoStore::default()));

        let bridge = Bridge::new(
            BridgeParts {
                seam: shell.seam,
                calls: shell.calls,
                watchers: shell.watchers,
                session: shell.session,
                domains: shell.domains,
            },
            &shell.waker,
        );

        let compositor = display.bind_on(&route, 1..=COMPOSITOR)?;
        let layer_shell = display.bind_on(&route, 1..=LAYER_SHELL)?;
        let seats = Seats::builder().bind(&display, &route);
        let screens = Screens::new(display.bind_on(&route, 1..=XDG_OUTPUTS).ok());
        display.announce_globals(&route);
        let locker = Locker::new(display.bind_on(&route, 1..=SESSION_LOCK).ok());
        let idling = Idling::new(shell.idles, display.bind_on(&route, 1..=IDLE_NOTIFIER).ok());
        let alpha_modifier = display.bind_optional(1..=ALPHA_MODIFIER);

        let host = rikka_wayland::Host::read_by_owner(display);
        let glass = Glass::bind(&host);
        let window_views = WindowViews::bind(&host);
        let surfaces = Surfaces::new(
            shell.name,
            shell.shared_theme,
            shell.surfaces,
            shell.host.outputs().clone(),
            Wayland {
                host,
                route,
                compositor,
                layer_shell,
                glass,
                alpha_modifier,
                window_views,
                locker,
            },
            embedder,
        );
        Ok(Self {
            screens,
            seats,
            surfaces,
            idling,
            host: shell.host,
            keyboard: shell.keyboard,
            bridge,
            tasks,
            timers: Timers::default(),
            polling: None,
            stopped: false,
            error: None,
        })
    }

    pub(crate) fn finish(mut self) -> Result<()> {
        self.surfaces.shutdown();
        match self.error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    #[must_use]
    pub(crate) fn stopped(&self) -> bool {
        self.stopped
    }

    pub(crate) fn fail(&mut self, error: Error) {
        tracing::error!("fatal error: {error}");
        if self.error.is_none() {
            self.error = Some(error);
        }
        self.stopped = true;
    }

    fn settle(&mut self, done: Result<()>) {
        if let Err(error) = done {
            self.fail(error);
        }
    }

    pub(super) fn handle_global(
        &mut self,
        registry: &WlRegistry,
        name: u32,
        interface: &str,
        version: u32,
    ) {
        if interface == WlOutput::INTERFACE.name {
            let output = registry.bind::<WlOutput>(name, version.min(OUTPUT));
            self.screens.add(name, output);
        } else if interface == WlSeat::INTERFACE.name {
            self.seats.add(registry, name, version);
        }
    }

    pub(super) fn handle_global_remove(&mut self, name: u32) {
        if self.screens.remove(name) {
            self.reconcile();
        } else {
            self.seats.remove(name);
        }
    }

    pub(super) fn follow_screen(&mut self, incoming: screens::OutputChange) {
        if incoming.settled {
            self.reconcile();
        }
        if incoming.rescaled {
            let done = self.surfaces.rescale_all(&self.screens);
            self.settle(done);
        }
    }

    pub(super) fn reconcile(&mut self) {
        self.surfaces.reconcile(&self.screens);
    }

    pub(super) fn handle_configure(&mut self, id: &ObjectId, serial: u32, suggested: (u32, u32)) {
        let done = self.surfaces.configure(id, serial, suggested);
        self.settle(done);
    }

    pub(super) fn handle_unmapped(&mut self, id: &ObjectId) {
        self.surfaces.unmapped(id);
    }

    pub(super) fn handle_unmap_sync(&mut self, sync: UnmapSync) {
        for callback in self.surfaces.take_unmap_callbacks(sync) {
            callback();
        }
    }

    pub(super) fn handle_closed(&mut self, id: &ObjectId) {
        self.surfaces.despawn(id);
    }

    pub(super) fn handle_surface_enter(&mut self, surface: &WlSurface, output: WlOutput) {
        let done = self.surfaces.entered(surface, output, &self.screens);
        self.settle(done);
    }

    pub(super) fn handle_surface_leave(&mut self, surface: &WlSurface, output: &WlOutput) {
        let done = self.surfaces.left(surface, output, &self.screens);
        self.settle(done);
    }

    pub(super) fn handle_scale_factor(&mut self, surface: &WlSurface, factor: i32) {
        let done = self.surfaces.set_scale(&surface.object_id(), factor);
        self.settle(done);
    }

    pub(super) fn handle_frame_callback(&mut self, id: &ObjectId) {
        if let Some(hosted) = self.surfaces.hosted_mut(id) {
            hosted.frame_answered();
        }
    }

    pub(super) fn read_mail(&mut self) {
        while let Ok((id, inbound)) = self.tasks.try_recv() {
            self.surfaces.deliver(&id, inbound);
        }
        self.bridge.read_mail(&mut self.host);
    }

    #[must_use]
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.timers
            .next()
            .into_iter()
            .chain(self.polling)
            .chain(self.seats.repeat_due())
            .min()
    }

    pub(crate) fn ring(&mut self, now: Instant) {
        if self.polling.is_some_and(|due| due <= now) {
            self.polling = None;
        }
        for alarm in self.timers.take_due(now) {
            match alarm {
                Alarm::ExitTimeout { desc, leave } => {
                    self.surfaces.finish_exit(desc, leave, &self.screens);
                }
            }
        }
        Seats::flush(self);
        Seats::repeat(self, now);
    }

    pub(super) fn about_to_wait(&mut self) {
        if self.error.is_some() {
            self.stopped = true;
            return;
        }
        self.bridge.announce(&mut self.host);
        self.follow_carets();
        self.idling.follow(self.seats.first(), &mut self.host);
        if let Some(typing) = self.seats.take_keyboard_change() {
            self.keyboard.settle(typing);
        }
        self.surfaces.follow_skin();
        self.surfaces
            .follow_presences(&mut self.timers, &self.screens);
        self.surfaces.follow_geometry(&self.screens);
        let outcome = self.surfaces.frame();
        self.surfaces.follow_opacities();
        self.surfaces.service(&mut self.seats);
        self.surfaces.follow_exits(&self.screens);
        self.surfaces.follow_unmaps();
        self.poll_while_busy(outcome.wake);
        if self.surfaces.is_done() {
            self.stopped = true;
        }
    }

    fn poll_while_busy(&mut self, wake: WakeHint) {
        let Some(delay) = wake.delay() else {
            return;
        };
        let due = Instant::now() + delay;
        if self.polling.is_some_and(|polling| polling <= due) {
            return;
        }
        self.polling = Some(due);
    }

    fn follow_carets(&mut self) {
        let surfaces = &self.surfaces;
        self.seats.update_text_input_rects(|surface| {
            surfaces
                .hosted(&surface.object_id())
                .and_then(|hosted| hosted.ui.caret())
        });
    }
}

impl SeatHost for ShellState {
    fn seats(&mut self) -> &mut Seats {
        &mut self.seats
    }

    fn handle_event(&mut self, incoming: SeatEvent) {
        let (surface, event, stylus) = match incoming {
            SeatEvent::Input { surface, event } => (surface, event, None),
            SeatEvent::Stylus {
                surface,
                event,
                stylus,
            } => (surface, event, Some(stylus)),
            SeatEvent::Keyboard { .. } | SeatEvent::Pad { .. } => return,
        };
        if let Some(hosted) = self.surfaces.taking_input(&surface.object_id()) {
            hosted.ui.handle_input(event.into(), stylus);
        }
    }
}

impl std::fmt::Debug for ShellState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellState")
            .field("surfaces", &self.surfaces)
            .field("bridge", &self.bridge)
            .field("stopped", &self.stopped)
            .finish_non_exhaustive()
    }
}
