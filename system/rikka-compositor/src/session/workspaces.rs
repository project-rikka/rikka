use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, SeamError, Service};
use rikka_session::workspaces::{
    CAPABILITY, CHANGED, COUNT, SHOW, STATE, Show, State, VERSION, exists,
};

use super::announced;
use crate::context::windows_ctx;
use crate::state::Compositor;

pub(crate) type Announced = announced::Announced<State>;

#[derive(Debug)]
pub(crate) struct WorkspacesService;

fn show(compositor: &mut Compositor, call: Call) {
    let show = match call.decode::<Show>() {
        Ok(show) => show,
        Err(error) => return call.fail(error),
    };
    if !exists(show.number) {
        return call.fail(SeamError::InvalidParams(format!(
            "no workspace {}: each output has {COUNT}",
            show.number
        )));
    }
    windows_ctx!(compositor).apply_show(show);
    call.done();
}

impl Service<Compositor> for WorkspacesService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Per-output workspaces")
            .method(
                MethodInfo::new(STATE)
                    .summary("Active workspace per output and which workspaces have windows"),
            )
            .method(
                MethodInfo::new(SHOW).summary(
                    "Show workspace `number` on `output`, or on the output under the pointer",
                ),
            )
            .event(
                EventInfo::new(CHANGED)
                    .summary("Workspace state changed")
                    .latest(),
            )
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            STATE => compositor.session.workspaces.answer(call),
            SHOW => show(compositor, call),
            _ => call.unknown_method(),
        }
    }
}
