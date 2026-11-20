use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service};
use rikka_session::remote::{CAPABILITY, CHANGED, DISCONNECT, STATUS, VERSION};

use crate::controller::Controller;

pub(crate) struct RemoteService;

impl Service<Controller> for RemoteService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Remote desktop: this session in the Rikka Remote viewer over QUIC")
            .method(MethodInfo::new(STATUS).summary("Where it listens and who is viewing"))
            .method(MethodInfo::new(DISCONNECT).summary("End the current viewer's connection"))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Listening or viewer changed")
                    .latest(),
            )
    }

    fn call(&mut self, controller: &mut Controller, method: Method, call: Call) {
        match &*method {
            STATUS => call.encode(controller.status()),
            DISCONNECT => controller.disconnect(call),
            _ => call.unknown_method(),
        }
    }
}
