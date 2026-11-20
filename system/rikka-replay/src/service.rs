use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service};
use rikka_session::replay::{CAPABILITY, CHANGED, DISCARD, DRAFT, KEEP, SAVE, STATUS, VERSION};

use crate::controller::Controller;

pub(crate) struct ReplayService;

impl Service<Controller> for ReplayService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Instant replay: recent screen capture kept in memory")
            .method(MethodInfo::new(STATUS).summary("Buffered subjects and their durations"))
            .method(
                MethodInfo::new(SAVE)
                    .summary("Save the last seconds of a subject, or of the focused one"),
            )
            .method(
                MethodInfo::new(DRAFT)
                    .summary("Write a subject's buffer to a draft file for trimming"),
            )
            .method(MethodInfo::new(KEEP).summary("Save a span of a draft"))
            .method(MethodInfo::new(DISCARD).summary("Delete a draft"))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Buffered subjects changed")
                    .latest(),
            )
    }

    fn call(&mut self, controller: &mut Controller, method: Method, call: Call) {
        match &*method {
            STATUS => call.encode(&controller.status()),
            SAVE => controller.save(call),
            DRAFT => controller.draft(call),
            KEEP => controller.keep(call),
            DISCARD => controller.discard(call),
            _ => call.unknown_method(),
        }
    }
}
