use rikka_bus::login::{self, ManagerProxy, SessionProxy};
use rikka_bus::{EventSink, Handler, Notifier, Rule, Service, Signals, StartError, Watch};
use std::convert::Infallible;
use zbus::Message;
use zbus::blocking::Connection;

const LOGIND: &str = "org.freedesktop.login1";
const SESSION: &str = "org.freedesktop.login1.Session";
const LOCK: &str = "Lock";
const UNLOCK: &str = "Unlock";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    Lock,
    Unlock,
}

#[derive(Debug)]
pub struct LoginSession(Service<Locking>);

impl LoginSession {
    pub fn watch(on_request: impl Fn(Asked) + Send + 'static) -> Result<LoginSession, StartError> {
        let asked: Box<dyn Fn(Asked) + Send> = Box::new(on_request);
        Service::start(asked, EventSink::new(|()| {}), |error| {
            tracing::warn!("failed to watch logind Lock/Unlock: {error}");
        })
        .map(LoginSession)
    }

    pub fn set_locked_hint(&self, locked: bool) {
        self.0.send(locked);
    }
}

pub(crate) struct Locking {
    session: SessionProxy<'static>,
    _watch: Watch,
}

impl Handler for Locking {
    type Config = Box<dyn Fn(Asked) + Send>;
    type Request = bool;
    type Notice = Infallible;
    type Event = ();
    type State = ();

    const THREAD_NAME: &'static str = "rikka-login-lock";
    const UNAVAILABLE: &'static str = "this session cannot follow logind Lock and Unlock";

    fn open(
        asked: Box<dyn Fn(Asked) + Send>,
        _notifier: Notifier<Locking>,
        _events: &EventSink<()>,
    ) -> Result<Locking, StartError> {
        let connection = Connection::system()?;
        let path = login::current_session(&ManagerProxy::new(&connection)?)?;
        let watch = Signals::new()
            .rule(
                Rule::new()
                    .sender(LOGIND)
                    .interface(SESSION)
                    .path(path.as_str()),
            )
            .each(&connection, ask, move |request| {
                asked(request);
                true
            })?;
        Ok(Locking {
            session: login::session_proxy(&connection, path)?,
            _watch: watch,
        })
    }

    fn handle_request(&mut self, locked: bool, _events: &EventSink<()>) {
        if let Err(error) = self.session.set_locked_hint(locked) {
            tracing::warn!("failed to set LockedHint: {error}");
        }
    }

    fn handle_notice(&mut self, notice: Infallible, _events: &EventSink<()>) {
        match notice {}
    }

    fn replaces(_pending: &bool, _next: &bool) -> bool {
        true
    }
}

fn ask(message: &Message) -> Option<Asked> {
    match message.header().member()?.as_str() {
        LOCK => Some(Asked::Lock),
        UNLOCK => Some(Asked::Unlock),
        _ => None,
    }
}
