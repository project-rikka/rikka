use rikka_compositor_input::Leds;

use crate::context::InputCtx;

impl InputCtx<'_> {
    pub(super) fn show_lock_leds(&mut self) {
        let Some(modifiers) = self
            .seats
            .get(self.input.seat)
            .map(|seat| seat.keyboard().modifiers())
        else {
            return;
        };
        let mut leds = Leds::empty();
        leds.set(Leds::CAPS_LOCK, modifiers.caps_lock);
        leds.set(Leds::NUM_LOCK, modifiers.num_lock);
        if std::mem::replace(&mut self.input.leds, leds) != leds {
            self.backend.show_leds(leds);
        }
    }
}
