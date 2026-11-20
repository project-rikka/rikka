use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, SeamError, Service};
use rikka_session::toplevels::{
    CAPABILITY, CHANGED, CLOSE, FOCUS, MAXIMIZE, MINIMIZE, MOVE, MoveRequest, STATE, State,
    ToplevelRef, UNMAXIMIZE, VERSION,
};
use rikka_session::workspaces::{COUNT, exists};

use super::announced;
use crate::context::windows_ctx;
use crate::state::Compositor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToplevelRequest {
    Focus,
    Close,
    Minimize,
    Maximize,
    Unmaximize,
    Move(usize),
}

pub(crate) type Announced = announced::Announced<State>;

#[derive(Debug)]
pub(crate) struct ToplevelsService;

fn apply(compositor: &mut Compositor, request: ToplevelRequest, call: Call) {
    let which = match call.decode::<ToplevelRef>() {
        Ok(which) => which,
        Err(error) => return call.fail(error),
    };
    windows_ctx!(compositor).apply_toplevel_request(request, &which.id);
    call.done();
}

fn send(compositor: &mut Compositor, call: Call) {
    let moved = match call.decode::<MoveRequest>() {
        Ok(moved) => moved,
        Err(error) => return call.fail(error),
    };
    if !exists(moved.workspace) {
        return call.fail(SeamError::InvalidParams(format!(
            "no workspace {}: each output has {COUNT}",
            moved.workspace
        )));
    }
    windows_ctx!(compositor)
        .apply_toplevel_request(ToplevelRequest::Move(moved.workspace), &moved.id);
    call.done();
}

impl Service<Compositor> for ToplevelsService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Open toplevel windows")
            .method(
                MethodInfo::new(STATE)
                    .summary("All windows with their bounds, output and workspace"),
            )
            .method(
                MethodInfo::new(FOCUS)
                    .summary("Focus window `id`, switching to its workspace if needed"),
            )
            .method(MethodInfo::new(CLOSE).summary("Close window `id`"))
            .method(MethodInfo::new(MINIMIZE).summary("Minimize floating window `id`"))
            .method(MethodInfo::new(MAXIMIZE).summary("Maximize window `id`"))
            .method(MethodInfo::new(UNMAXIMIZE).summary("Restore maximized window `id`"))
            .method(MethodInfo::new(MOVE).summary("Move window `id` to `workspace` on its output"))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Toplevel list changed")
                    .latest(),
            )
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            STATE => compositor.session.toplevels.answer(call),
            FOCUS => apply(compositor, ToplevelRequest::Focus, call),
            CLOSE => apply(compositor, ToplevelRequest::Close, call),
            MINIMIZE => apply(compositor, ToplevelRequest::Minimize, call),
            MAXIMIZE => apply(compositor, ToplevelRequest::Maximize, call),
            UNMAXIMIZE => apply(compositor, ToplevelRequest::Unmaximize, call),
            MOVE => send(compositor, call),
            _ => call.unknown_method(),
        }
    }
}
