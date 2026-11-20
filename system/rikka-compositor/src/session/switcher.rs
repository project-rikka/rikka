use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service};
use rikka_session::switcher::{
    ARRANGE, Arrangement, CAPABILITY, CHANGED, CHOOSE, DISMISS, Dismissal, Pick, SELECT, STATE,
    Switch, VERSION,
};
use serde::de::DeserializeOwned;

use super::announced;
use crate::context::windows_ctx;
use crate::state::Compositor;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SwitcherRequest {
    Select(Pick),
    Choose(Pick),
    Dismiss(Dismissal),
    Arrange(Arrangement),
}

impl SwitcherRequest {
    pub(crate) fn serial(&self) -> u64 {
        match self {
            SwitcherRequest::Select(pick) | SwitcherRequest::Choose(pick) => pick.serial,
            SwitcherRequest::Dismiss(dismissal) => dismissal.serial,
            SwitcherRequest::Arrange(arrangement) => arrangement.serial,
        }
    }
}

pub(crate) type Switches = announced::Announced<Option<Switch>>;

#[derive(Debug)]
pub(crate) struct SwitcherService;

fn apply<T: DeserializeOwned>(
    compositor: &mut Compositor,
    call: Call,
    request: impl FnOnce(T) -> SwitcherRequest,
) {
    match call.decode::<T>() {
        Ok(body) => {
            windows_ctx!(compositor).apply_switcher_request(request(body));
            call.done();
        }
        Err(error) => call.fail(error),
    }
}

impl Service<Compositor> for SwitcherService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Alt+Tab window switcher")
            .method(
                MethodInfo::new(STATE)
                    .summary("Switcher windows in MRU order and the selected one"),
            )
            .method(MethodInfo::new(SELECT).summary("Select window `id` in switch `serial`"))
            .method(
                MethodInfo::new(CHOOSE)
                    .summary("Activate window `id` in switch `serial` and close the switcher"),
            )
            .method(
                MethodInfo::new(DISMISS).summary("Close switch `serial` without changing focus"),
            )
            .method(MethodInfo::new(ARRANGE).summary(
                "Report card centres of switch `serial` row by row for vertical navigation",
            ))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Switcher opened, changed selection or closed")
                    .latest(),
            )
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            STATE => compositor.session.switches.answer(call),
            SELECT => apply(compositor, call, SwitcherRequest::Select),
            CHOOSE => apply(compositor, call, SwitcherRequest::Choose),
            DISMISS => apply(compositor, call, SwitcherRequest::Dismiss),
            ARRANGE => apply(compositor, call, SwitcherRequest::Arrange),
            _ => call.unknown_method(),
        }
    }
}
