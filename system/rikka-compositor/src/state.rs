use std::any::Any;
use std::fmt;
use std::time::Instant;

use rikka_compositor_desktop::WindowId;
use rikka_compositor_output::{BackgroundEffects, Captures};
use rikka_compositor_reactor::Reactor;
use rikka_compositor_seat::Seats;
use rikka_compositor_server::{DisplayHandle, Socket};
use rikka_compositor_shell::Shell;
use rikka_compositor_surface::Surfaces;
use rikka_session::profile::Profile;

use crate::animation::Animations;
use crate::backend::Backend;
use crate::capture::Capture;
use crate::context::{
    CaptureCtx, capture_ctx, input_ctx, lock_ctx, parts, session_ref, windows_ctx,
};
use crate::cursor::Cursor;
use crate::error::{Error, Result};
use crate::input::Input;
use crate::outputs::Outputs;
use crate::policy::Policy;
use crate::render::{FrameScheduler, FrameTargets, Resting, Scenes, Snapshot, Throttle};
use crate::selection::Selection;
use crate::session::Session;
use crate::settings::SessionSettings;
use crate::shell::Locking;
use crate::snap::Snapping;
use crate::switcher::Switcher;
use crate::wayland::{Leasing, Presence, Sandboxes, Views, WorkspaceList};
use crate::windows::Windows;
use crate::x11::X11;

pub struct Compositor {
    pub(crate) core: Core,
    pub(crate) backend: Box<dyn Backend>,
    pub(crate) outputs: Outputs,
    pub(crate) surfaces: Surfaces,
    pub(crate) shell: Shell,
    pub(crate) seats: Seats,
    pub(crate) windows: Windows,
    pub(crate) locking: Locking,
    pub(crate) presence: Presence,
    pub(crate) frames: FrameScheduler,
    pub(crate) scenes: Scenes,
    pub(crate) throttle: Throttle,
    pub(crate) resting: Resting,
    pub(crate) captures: Captures<WindowId>,
    pub(crate) capture: Capture,
    pub(crate) effects: BackgroundEffects,
    pub(crate) leasing: Leasing,
    pub(crate) sandboxes: Sandboxes,
    pub(crate) workspace_list: WorkspaceList,
    pub(crate) views: Views,
    pub(crate) cursor: Cursor,
    pub(crate) input: Input,
    pub(crate) selection: Selection,
    pub(crate) policy: Policy,
    pub(crate) animations: Animations,
    pub(crate) snap: Snapping,
    pub(crate) switcher: Switcher,
    pub(crate) settings: Option<SessionSettings>,
    pub(crate) session: Session,
    pub(crate) x11: X11,
}

pub(crate) struct Core {
    pub(crate) reactor: Reactor<Compositor>,
    pub(crate) display: DisplayHandle<Compositor>,
    pub(crate) socket: Socket,
    pub(crate) profile: Profile,
    pub(crate) running: bool,
}

impl Core {
    pub(crate) fn socket_name(&self) -> &str {
        self.socket.name()
    }

    pub(crate) fn stop(&mut self) {
        self.running = false;
    }
}

pub(crate) struct Parts<'a> {
    pub(crate) core: &'a mut Core,
    pub(crate) outputs: &'a mut Outputs,
    pub(crate) surfaces: &'a mut Surfaces,
    pub(crate) windows: &'a Windows,
    pub(crate) frames: &'a mut FrameScheduler,
    pub(crate) scenes: &'a mut Scenes,
    pub(crate) throttle: &'a mut Throttle,
    pub(crate) resting: &'a mut Resting,
    pub(crate) cursor: &'a mut Cursor,
    pub(crate) input: &'a mut Input,
    pub(crate) selection: &'a Selection,
    pub(crate) policy: &'a Policy,
    pub(crate) animations: &'a mut Animations,
    pub(crate) snap: &'a mut Snapping,
}

impl Compositor {
    pub(crate) fn split(&mut self) -> (&mut dyn Backend, Parts<'_>) {
        (&mut *self.backend, parts!(self))
    }

    pub(crate) fn settle(&mut self) {
        while rikka_compositor_seat::flush(self) | rikka_compositor_selection::flush(self) {}
    }

    pub(crate) fn backend<B: Backend>(&mut self) -> Option<(&mut B, Parts<'_>)> {
        let (backend, parts) = self.split();
        let backend: &mut dyn Any = backend;
        Some((backend.downcast_mut::<B>()?, parts))
    }

    pub fn socket_name(&self) -> &str {
        self.core.socket_name()
    }

    pub fn reactor(&self) -> &Reactor<Compositor> {
        &self.core.reactor
    }

    pub fn run(&mut self) -> Result<()> {
        let reactor = self.core.reactor.clone();
        let ran = reactor.run(self, |compositor| !compositor.core.running);
        session_ref!(self).retire_session_environment();
        tracing::info!("compositor shutting down");
        ran.map_err(Error::EventLoop)
    }

    pub fn stop(&mut self) {
        self.core.stop();
    }

    pub fn x_display(&self) -> Option<u32> {
        self.x11.display_number()
    }

    pub fn snapshot(&mut self) -> Option<Snapshot> {
        let output = self.outputs.enabled().next()?.id();
        self.backend.snapshot(output)
    }

    pub(crate) fn each_turn(&mut self) {
        windows_ctx!(self).flush_configures();
        windows_ctx!(self).announce_session(&mut self.workspace_list);
        input_ctx!(self).input_turn();
        self.settle();
        capture_ctx!(self).place_capture_cursors();
        windows_ctx!(self).flush_x11();
    }

    pub(crate) fn dispatch_frames(&mut self) {
        self.dispatch_pending_input();
        input_ctx!(self).apply_grab_requests();
        windows_ctx!(self).finish_animations();
        input_ctx!(self).drag_icon_followed();
        input_ctx!(self).run_key_actions();
        self.settle();
        let changed = self.frames.take_changed();
        if changed {
            self.presence
                .update(&self.outputs, &self.windows, &mut self.surfaces);
        }
        windows_ctx!(self).pointer_follows_surfaces();
        if changed {
            let at = self.cursor.position();
            input_ctx!(self).caption_follows_pointer(at);
        }
        self.settle();
        while let Some((output, next_presentation)) = self.frames.next_refreshed() {
            rikka_compositor_surface::refreshed(self, output, next_presentation);
            self.settle();
        }
        while let Some(output) = self.frames.next_finished() {
            capture_ctx!(self).frame_finished(output);
            if self.outputs.wayland().is_published(output) {
                lock_ctx!(self).lock_shown_on(output);
            }
            self.settle();
        }
        match self.frames.take() {
            None => {}
            Some(FrameTargets::Every) => {
                for output in self.outputs.enabled() {
                    self.backend.schedule(output.id());
                }
            }
            Some(FrameTargets::Named(named)) => {
                for output in named {
                    self.backend.schedule(*output);
                }
            }
        }
        self.frames.scheduled();
    }
}

impl fmt::Debug for Compositor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Compositor")
            .field("socket", &self.core.socket_name())
            .field("profile", &self.core.profile)
            .field("outputs", &self.outputs)
            .field("windows", &self.windows)
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Parts<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Parts")
            .field("outputs", &self.outputs)
            .finish_non_exhaustive()
    }
}

impl CaptureCtx<'_> {
    pub(crate) fn gpu_reached(&mut self, finished: u64, at: Instant) {
        self.scenes.gpu_reached(finished, self.surfaces);
        self.capture_read(finished);
        self.backend.gpu_reached(finished, at);
    }
}
