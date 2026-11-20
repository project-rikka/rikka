use rikka_compositor_seat::set_idle_inhibited;

use crate::context::SwitcherCtx;
use crate::context::seat_parts;

impl SwitcherCtx<'_> {
    pub(crate) fn judge_idle_inhibitors(&mut self) {
        let inhibited = self
            .seats
            .idle_inhibitors()
            .any(|surface| self.surfaces.get(surface).is_some());
        if inhibited != self.seats.idle_inhibited() {
            tracing::debug!("idle inhibited: {}", !inhibited);
            set_idle_inhibited(&mut seat_parts!(self), inhibited);
        }
    }
}
