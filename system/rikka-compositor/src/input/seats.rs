use rikka_compositor_seat::{Capabilities, SeatMut};
use rikka_ime::Caret;
use rikka_session::agent;

use super::keymap::REPEAT;
use crate::agent::AgentService;
use crate::caret::CaretService;
use crate::context::{SwitcherCtx, agent_seat, person};
use crate::state::Compositor;

pub(crate) const PERSON_SEAT: &str = "seat-0";

impl Compositor {
    pub(crate) fn person(&mut self) -> SeatMut<'_, Compositor> {
        self.settle();
        let seat = self.input.seat;
        rikka_compositor_seat::seat(self, seat)
    }

    pub(crate) fn agent_seat(&mut self) -> SeatMut<'_, Compositor> {
        self.settle();
        let seat = self.input.agent.seat();
        rikka_compositor_seat::seat(self, seat)
    }
}

impl SwitcherCtx<'_> {
    pub(crate) fn seat_up(&mut self) {
        let mut person = person!(self);
        person
            .set_capabilities(Capabilities::POINTER | Capabilities::KEYBOARD | Capabilities::TOUCH);
        person.set_repeat(REPEAT);
        agent_seat!(self).set_repeat(REPEAT);
    }

    pub(crate) fn offer_input_seams(&mut self) {
        self.session.seam.register(agent::CAPABILITY, AgentService);
        if let Some(context) = self.session.seam.register(Caret::CAPABILITY, CaretService) {
            self.input.caret.attach(context.publisher().clone());
        }
    }
}
