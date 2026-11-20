use rikka_seam::{Call, Manifest, Method, MethodInfo, Service};
use rikka_session::agent::{CAPABILITY, PLACEMENT, SHOW, VERSION, WAIT_IDLE, WaitIdleRequest};

use crate::context::input_ctx;
use crate::state::Compositor;

#[derive(Debug)]
pub(crate) struct AgentService;

impl Service<Compositor> for AgentService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Agent pointer, separate from the user's pointer")
            .method(
                MethodInfo::new(PLACEMENT)
                    .summary("Output under the agent pointer, its size and the pointer position"),
            )
            .method(MethodInfo::new(SHOW).summary("Show or hide the agent pointer"))
            .method(MethodInfo::new(WAIT_IDLE).summary(
                "Answer once the agent's output has presented no frame for the quiet period, \
                 or at the timeout",
            ))
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            PLACEMENT => input_ctx!(compositor).answer_agent_placement(call),
            SHOW => match call.decode::<bool>() {
                Ok(visible) => {
                    input_ctx!(compositor).set_agent_visible(visible);
                    call.done();
                }
                Err(error) => call.fail(error),
            },
            WAIT_IDLE => match call.decode::<WaitIdleRequest>() {
                Ok(request) => {
                    let (_, responder) = call.defer();
                    input_ctx!(compositor).wait_agent_idle(responder, request);
                }
                Err(error) => call.fail(error),
            },
            _ => call.unknown_method(),
        }
    }
}
