use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service};
use rikka_session::pointer::{CAPABILITY, CHANGED, PointerMode, STATE, VERSION};

use super::announced;
use crate::state::Compositor;

pub(crate) type Pointing = announced::Announced<PointerMode>;

#[derive(Debug)]
pub(crate) struct PointerService;

impl Service<Compositor> for PointerService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Pointer")
            .method(
                MethodInfo::new(STATE)
                    .summary("Whether an application holds the pointer for relative motion"),
            )
            .event(
                EventInfo::new(CHANGED)
                    .summary("An application took or released the pointer")
                    .latest(),
            )
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            STATE => compositor.session.pointing.answer(call),
            _ => call.unknown_method(),
        }
    }
}
