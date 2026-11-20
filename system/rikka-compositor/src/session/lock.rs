use rikka_power::LoginSession;
use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Publisher, Service};
use rikka_session::lock::{CAPABILITY, CHANGED, LOCK_REQUESTED, LockState, STATE, VERSION};

use crate::state::Compositor;

#[derive(Debug, Default)]
pub(crate) struct Guarded {
    state: LockState,
    publisher: Option<Publisher>,
    login: Option<LoginSession>,
    hinted: Option<bool>,
}

impl Guarded {
    pub(crate) fn new(login: Option<LoginSession>) -> Self {
        let mut guarded = Guarded {
            login,
            ..Guarded::default()
        };
        guarded.hint();
        guarded
    }

    pub(crate) fn attach(&mut self, publisher: Publisher) {
        self.publisher = Some(publisher);
    }

    pub(crate) fn set(&mut self, state: LockState) {
        if std::mem::replace(&mut self.state, state) == state {
            return;
        }
        self.hint();
        self.publish(CHANGED, &state);
    }

    pub(crate) fn get(&self) -> LockState {
        self.state
    }

    pub(crate) fn request_locker(&self) {
        self.publish(LOCK_REQUESTED, &());
    }

    fn publish(&self, event: &str, payload: &impl serde::Serialize) {
        let Some(publisher) = &self.publisher else {
            return;
        };
        if let Err(error) = publisher.emit(event, payload) {
            tracing::warn!(%error, event, "failed to publish session lock event");
        }
    }

    fn hint(&mut self) {
        let Some(login) = &self.login else {
            return;
        };
        let locked = match self.state {
            LockState::Open => false,
            LockState::Locking => return,
            LockState::Locked | LockState::Sealed | LockState::Abandoned => true,
        };
        if self.hinted.replace(locked) != Some(locked) {
            login.set_locked_hint(locked);
        }
    }
}

#[derive(Debug)]
pub(crate) struct LockService;

impl Service<Compositor> for LockService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Session lock state")
            .method(MethodInfo::new(STATE).summary(
                "Lock state: open, locking, locked, sealed (no locker yet) or abandoned (locker exited)",
            ))
            .event(EventInfo::new(LOCK_REQUESTED).summary("logind requested a screen lock"))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Lock state changed; carries the new state")
                    .latest(),
            )
    }

    fn call(&mut self, compositor: &mut Compositor, method: Method, call: Call) {
        match &*method {
            STATE => call.encode(&compositor.session.guarded.get()),
            _ => call.unknown_method(),
        }
    }
}
