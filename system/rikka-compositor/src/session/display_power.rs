use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service};
use rikka_session::display_power::{CAPABILITY, CHANGED, DisplayPower, SLEEP, STATE, VERSION};

use super::announced;
use crate::context::lock_ctx;
use crate::state::Compositor;

pub(crate) type Slumber = announced::Announced<DisplayPower>;

#[derive(Debug)]
pub(crate) struct DisplayPowerService;

impl Service<Compositor> for DisplayPowerService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Display power")
            .method(MethodInfo::new(STATE).summary("Whether the displays are awake or asleep"))
            .method(
                MethodInfo::new(SLEEP)
                    .summary("Turn every display off until the person next touches an input"),
            )
            .event(
                EventInfo::new(CHANGED)
                    .summary("The displays woke or went to sleep")
                    .latest(),
            )
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            STATE => compositor.session.slumber.answer(call),
            SLEEP => {
                lock_ctx!(compositor).sleep_displays();
                call.done();
            }
            _ => call.unknown_method(),
        }
    }
}
