use std::time::{Duration, Instant};

use rikka_wayland_client::Proxy;

use super::{SurfaceId, Surfaces, taking};
use crate::convert;
use crate::opacity::AlphaMultiplier;
use crate::screens::Screens;
use crate::timers::{Alarm, Timers};
use shell_kit::KeyboardMode;
use shell_kit::hosting::OutputTarget;

const FRAME_CALLBACK_SLACK: Duration = Duration::from_millis(80);

impl Surfaces {
    pub(crate) fn follow_presences(&mut self, timers: &mut Timers, screens: &Screens) {
        let moved: Vec<SurfaceId> = self
            .descs
            .iter()
            .filter(|(_, surface)| surface.settings.presence_moved())
            .map(|(desc, _)| desc)
            .collect();
        if moved.is_empty() {
            return;
        }
        for desc in moved {
            self.follow_presence(desc, timers, screens);
        }
        self.reconcile(screens);
    }

    fn follow_presence(&mut self, desc: SurfaceId, timers: &mut Timers, screens: &Screens) {
        if self.descs[desc].settings.is_wanted() {
            let stays_put = self.stays_put(desc, screens);
            if self.descs[desc].is_leaving() {
                self.descs[desc].stop_leaving();
                match stays_put {
                    true => self.follow_leaving(desc),
                    false => self.despawn_all(desc),
                }
            } else if !stays_put {
                self.despawn_all(desc);
            }
            return;
        }
        if let OutputTarget::Aimed(aimed) = &self.descs[desc].settings.output {
            aimed.unpin();
        }
        if !self.spawned_anywhere(desc) {
            return;
        }
        if let Some(on_leave) = self.descs[desc].on_leave.clone() {
            self.run_on(desc, &on_leave);
        }
        let Some(exit) = self.descs[desc].exit() else {
            return;
        };
        let leave = self.descs[desc].start_leaving();
        timers.at(
            Instant::now() + exit + FRAME_CALLBACK_SLACK,
            Alarm::ExitTimeout { desc, leave },
        );
        self.follow_leaving(desc);
    }

    fn stays_put(&self, desc: SurfaceId, screens: &Screens) -> bool {
        match &self.descs[desc].settings.output {
            OutputTarget::Auto | OutputTarget::All => true,
            OutputTarget::Aimed(_) => false,
            OutputTarget::Destined(destination) => {
                let output = destination
                    .output()
                    .and_then(|name| screens.named(&name))
                    .map(Proxy::object_id);
                self.copies(desc)
                    .all(|hosted| hosted.state.output == output)
            }
        }
    }

    fn follow_leaving(&mut self, desc: SurfaceId) {
        self.retake(desc);
        let leaving = self.descs[desc].is_leaving();
        let keyboard = match leaving {
            true => KeyboardMode::None,
            false => self.descs[desc].settings.keyboard,
        };
        for hosted in self.copies_mut(desc) {
            if let Some(layer) = hosted.state.role.layer() {
                layer.set_keyboard_interactivity(convert::keyboard_mode(keyboard));
            }
            match leaving {
                true => hosted.ui.leave(),
                false => hosted.ui.rejoin(),
            }
        }
    }

    fn retake(&self, desc: SurfaceId) {
        let pointer = self.descs[desc].pointer();
        for hosted in self.copies(desc) {
            let taken = taking::taking(pointer, hosted.state.logical, &hosted.state.panes);
            taking::take(&self.compositor, hosted.state.role.surface(), &taken);
        }
    }

    pub(crate) fn finish_exit(&mut self, desc: SurfaceId, leave: u64, screens: &Screens) {
        if self.descs[desc].finish_leaving(leave) {
            self.reconcile(screens);
        }
    }

    pub(crate) fn follow_exits(&mut self, screens: &Screens) {
        let played_out: Vec<SurfaceId> = self
            .descs
            .iter()
            .filter(|(desc, surface)| surface.is_leaving() && self.played_out(*desc))
            .map(|(desc, _)| desc)
            .collect();
        if played_out.is_empty() {
            return;
        }
        for desc in played_out {
            self.descs[desc].stop_leaving();
        }
        self.reconcile(screens);
    }

    fn played_out(&self, desc: SurfaceId) -> bool {
        self.copies(desc).all(|hosted| {
            !hosted.ui.needs_render()
                && !hosted.awaits_frame()
                && hosted
                    .state
                    .alpha
                    .as_ref()
                    .is_none_or(AlphaMultiplier::is_clear)
        })
    }
}
