use std::collections::HashSet;
use std::time::{Duration, Instant};

use rikka_compositor_desktop::{Drawn, Target};
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_shell::{LockEvent, LockId, LockSurfaceId};
use rikka_compositor_types::OutputId;
use rikka_power::Asked;
use rikka_session::lock::LockState;

use crate::animation::{Lowered, recedes};
use crate::context::switcher_ctx;
use crate::context::{LockCtx, WindowsCtx, lock_ctx, session_ctx, windows_ctx, windows_ref};
use crate::glass::frosted;
use crate::session::LOCKER_TIMEOUT;
use crate::state::Compositor;

const LOCK_TRANSITION_TIMEOUT: Duration = Duration::from_secs(1);

const LOGIND_LOCK_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub(crate) struct Locking {
    awaiting: Option<LockId>,
    shown: Vec<OutputId>,
    logind_request: Option<Instant>,
}

impl LockCtx<'_> {
    fn logind_lock(&mut self) {
        if self.windows.desktop.locked() || self.locking.logind_request.is_some() {
            return;
        }
        tracing::info!("logind requested lock");
        let requested_at = Instant::now();
        self.locking.logind_request = Some(requested_at);
        self.session.guarded.request_locker();
        self.core
            .reactor
            .after(LOGIND_LOCK_TIMEOUT, move |compositor: &mut Compositor| {
                if compositor.locking.logind_request != Some(requested_at) {
                    return;
                }
                tracing::warn!("no screen locker responded to logind; sealing session");
                lock_ctx!(compositor).set_session_locked();
                session_ctx!(compositor).await_locker(LOCKER_TIMEOUT);
            });
    }

    pub(crate) fn lock_shown_on(&mut self, output: OutputId) {
        if self.locking.awaiting.is_none() {
            return;
        }
        let drawn = self
            .windows
            .desktop
            .lock_surface_on(output)
            .and_then(|(_, surface)| self.surfaces.get(surface))
            .is_some_and(|surface| surface.buffer_attached());
        if drawn
            && self.animations.lock_transition.covers(output)
            && !self.locking.shown.contains(&output)
        {
            self.locking.shown.push(output);
        }
        self.confirm_lock();
    }

    pub(crate) fn confirm_lock(&mut self) {
        let Some(lock) = self.locking.awaiting else {
            return;
        };
        let everywhere = self.session.displays_asleep()
            || self
                .windows
                .desktop
                .outputs()
                .all(|(id, _)| self.locking.shown.contains(&id));
        if everywhere && self.shell.lock_session(lock) {
            self.locking.awaiting = None;
            self.session.guarded.set(LockState::Locked);
            self.animations.lock_transition.cancel_lowering();
            windows_ctx!(self).finish_animations();
            tracing::info!("session locked on all outputs");
        }
    }

    pub(crate) fn lock_event(&mut self, event: LockEvent) {
        match event {
            LockEvent::Requested(lock) => self.lock_requested(lock),
            LockEvent::Abandoned(lock) | LockEvent::Unlocked(lock) => self.unlock(lock),
            LockEvent::LockerGone(_) => {
                tracing::warn!("screen locker exited; session stays locked");
                self.session.guarded.set(LockState::Abandoned);
                session_ctx!(self).await_locker(LOCKER_TIMEOUT);
            }
            LockEvent::SurfaceCreated(surface) => windows_ctx!(self).guard(surface),
            LockEvent::SurfaceDestroyed(surface) => {
                self.windows.desktop.remove_lock_surface(surface);
                self.frames.request_all();
            }
        }
    }

    pub(crate) fn set_session_locked(&mut self) {
        if self.windows.desktop.locked() {
            return;
        }
        self.animations.lock_transition.drop_all();
        self.shell.set_session_locked();
        *self.locking = Locking::default();
        windows_ctx!(self).conceal_desktop(LockState::Sealed);
        self.frames.request_all();
        tracing::info!("session sealed until a screen locker attaches");
    }

    pub(crate) fn logind_request(&mut self, request: Asked) {
        match request {
            Asked::Lock => self.logind_lock(),
            Asked::Unlock => self.logind_unlock(),
        }
    }

    fn logind_unlock(&mut self) {
        self.locking.logind_request = None;
        if !self.windows.desktop.locked() {
            return;
        }
        tracing::info!("logind requested unlock");
        self.shell.unlock_session();
        self.unlock_session();
    }

    fn lock_requested(&mut self, lock: LockId) {
        windows_ctx!(self).begin_lock_transition();
        *self.locking = Locking {
            awaiting: Some(lock),
            ..Locking::default()
        };
        windows_ctx!(self).conceal_desktop(LockState::Locking);
        self.core
            .reactor
            .after(LOCK_TRANSITION_TIMEOUT, |compositor: &mut Compositor| {
                compositor.animations.lock_transition.cancel_lowering();
                windows_ctx!(compositor).finish_animations();
            });
        tracing::info!("session locking");
        if self.session.displays_asleep() {
            self.confirm_lock();
        }
    }

    fn unlock(&mut self, lock: LockId) {
        self.shell.finish_lock(lock);
        if !self.windows.desktop.locked() {
            *self.locking = Locking::default();
            return;
        }
        self.unlock_session();
    }

    fn unlock_session(&mut self) {
        *self.locking = Locking::default();
        windows_ctx!(self).begin_unlock_transition();
        self.windows.desktop.unlock();
        self.session.guarded.set(LockState::Open);
        windows_ctx!(self).refocus(None);
        windows_ctx!(self).desktop_changed();
        tracing::info!("session unlocked");
    }
}

impl WindowsCtx<'_> {
    fn begin_lock_transition(&mut self) {
        if !self.animations.enabled() {
            return;
        }
        let mut frozen_roots = HashSet::new();
        let mut lowered = Vec::new();
        let screens: Vec<_> = self.outputs.enabled().map(|output| output.id()).collect();
        for screen in screens {
            let Some(composed) = self.scenes.composed(screen) else {
                continue;
            };
            let drawn: Vec<Drawn> = composed.drawn.clone();
            for item in drawn {
                let Drawn::Surface {
                    target,
                    root,
                    location,
                } = item
                else {
                    continue;
                };
                if matches!(target, Target::Lock(_)) || !frozen_roots.insert(root) {
                    continue;
                }
                let Some(mut bounds) = self.surfaces.bounding_box(root) else {
                    continue;
                };
                bounds.loc += location;
                let frost = frosted(self.surfaces, root, location - bounds.loc);
                let snapshot = self.animations.mint();
                if self.scenes.freeze(root, snapshot, bounds, &frost) {
                    lowered.push(Lowered {
                        snapshot,
                        rect: bounds,
                        recedes: recedes(&self.windows.desktop, target),
                    });
                }
            }
        }
        self.animations.lock_transition.lower(lowered);
    }

    fn begin_unlock_transition(&mut self) {
        if !self.animations.enabled() {
            return self.animations.lock_transition.drop_all();
        }
        let outputs: Vec<(OutputId, Rect<i32, Logical>)> = self
            .windows
            .desktop
            .outputs()
            .map(|(id, output)| (id, output.geometry()))
            .collect();
        for (output, area) in outputs {
            let guard = self
                .windows
                .desktop
                .lock_surface_on(output)
                .map(|(_, surface)| surface);
            let lock = guard.and_then(|root| {
                let snapshot = self.animations.mint();
                self.scenes
                    .freeze(root, snapshot, area, &[])
                    .then_some((snapshot, area))
            });
            self.animations.lock_transition.raise(output, lock);
        }
        self.frames.request_all();
    }

    fn conceal_desktop(&mut self, state: LockState) {
        self.windows.desktop.lock();
        self.session.guarded.set(state);
        self.set_keyboard_focus(None);
        switcher_ctx!(self).abandon_switch();
        self.desktop_changed();
    }

    fn guard(&mut self, surface: LockSurfaceId) {
        let Some(state) = self.shell.lock_surface(surface) else {
            return;
        };
        let root = state.surface();
        let Some(output) = self.outputs.wayland().output_of(state.output()) else {
            return;
        };
        if let Err(error) = self.windows.desktop.add_lock_surface(surface, output, root) {
            return tracing::warn!("cannot map lock surface: {error}");
        }
        let size = self.windows.full_area(output).size;
        self.shell.configure_lock_surface(surface, size);
        if windows_ref!(self).active_output() == Some(output)
            || windows_ref!(self).keyboard_focus().is_none()
        {
            self.focus_surface(root);
        }
        self.frames.request_all();
    }

    pub(crate) fn focus_lock_surface(&mut self) {
        let Some(output) = windows_ref!(self).active_output() else {
            return;
        };
        if let Some((_, surface)) = self.windows.desktop.lock_surface_on(output) {
            self.focus_surface(surface);
        }
    }
}
