use rikka_session::display_power::DisplayPower;

use crate::context::parts;
use crate::context::{InputCtx, LockCtx};
use crate::session::Session;

impl Session {
    pub(crate) fn displays_asleep(&self) -> bool {
        *self.slumber.current() == DisplayPower::Asleep
    }
}

impl InputCtx<'_> {
    pub(crate) fn wake_displays(&mut self) {
        if !self.session.displays_asleep() {
            return;
        }
        self.backend.wake_displays(&mut parts!(self));
        self.session.slumber.publish(DisplayPower::Awake);
        self.frames.request_all();
        tracing::info!("displays woke");
    }
}

impl LockCtx<'_> {
    pub(crate) fn sleep_displays(&mut self) {
        if self.session.displays_asleep() {
            return;
        }
        let slept = self.backend.sleep_displays(&mut parts!(self));
        if !slept {
            return;
        }
        self.session.slumber.publish(DisplayPower::Asleep);
        tracing::info!("displays asleep until input");
        self.confirm_lock();
    }
}
