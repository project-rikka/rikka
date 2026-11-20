use rikka_ime::Caret;
use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service};

use crate::context::input_ref;
use crate::session::Announced;
use crate::state::Compositor;

pub(super) type Aims = Announced<Option<Caret>>;

#[derive(Debug)]
pub(crate) struct CaretService;

impl Service<Compositor> for CaretService {
    fn manifest(&self) -> Manifest {
        Manifest::new(Caret::CAPABILITY, Caret::VERSION)
            .summary("Text-input caret of the focused client")
            .method(
                MethodInfo::new(Caret::WHERE)
                    .summary("Caret rectangle, or none when no text field is focused"),
            )
            .method(
                MethodInfo::new(Caret::AIM)
                    .summary("Anchor for typing UI: the caret, else the pointer, with its output"),
            )
            .event(
                EventInfo::new(Caret::MOVED)
                    .summary("Caret moved, disappeared or changed window")
                    .latest(),
            )
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            Caret::WHERE => compositor.input.caret.aims.answer(call),
            Caret::AIM => call.encode(&input_ref!(compositor).aim()),
            _ => call.unknown_method(),
        }
    }
}
