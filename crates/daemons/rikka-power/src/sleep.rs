use std::convert::Infallible;
use std::os::fd::OwnedFd;

use rikka_bus::login::ManagerProxy;
use rikka_bus::{EventSink, Handler, Notifier, Rule, Service, Signals, StartError, Watch};
use zbus::Message;
use zbus::blocking::Connection;

const LOGIND: &str = "org.freedesktop.login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const PREPARE_FOR_SLEEP: &str = "PrepareForSleep";
const WHAT: &str = "sleep";
const MODE: &str = "delay";

#[derive(Debug)]
pub enum Sleep {
    Going(Delay),
    Woke,
}

#[derive(Debug)]
pub struct Delay {
    _inhibitor: Option<OwnedFd>,
}

impl Delay {
    pub fn release(self) {}
}

#[derive(Debug)]
pub struct SleepWatch(
    #[expect(dead_code, reason = "dropping the service ends the watch")] Service<Sleeping>,
);

impl SleepWatch {
    pub fn start(
        who: impl Into<String>,
        why: impl Into<String>,
        on_sleep: impl FnMut(Sleep) + Send + 'static,
    ) -> Result<SleepWatch, StartError> {
        let inhibitor = Inhibitor {
            who: who.into(),
            why: why.into(),
            on_sleep: Box::new(on_sleep),
        };
        Service::start(inhibitor, EventSink::new(|()| {}), |error| {
            tracing::warn!("failed to watch PrepareForSleep: {error}");
        })
        .map(SleepWatch)
    }
}

pub(crate) struct Inhibitor {
    who: String,
    why: String,
    on_sleep: Box<dyn FnMut(Sleep) + Send>,
}

pub(crate) struct Sleeping {
    manager: ManagerProxy<'static>,
    inhibitor: Inhibitor,
    held: Option<OwnedFd>,
    _watch: Watch,
}

impl Handler for Sleeping {
    type Config = Inhibitor;
    type Request = Infallible;
    type Notice = bool;
    type Event = ();
    type State = ();

    const THREAD_NAME: &'static str = "rikka-sleep";
    const UNAVAILABLE: &'static str = "this session cannot delay sleep";

    fn open(
        inhibitor: Inhibitor,
        notifier: Notifier<Sleeping>,
        _events: &EventSink<()>,
    ) -> Result<Sleeping, StartError> {
        let connection = Connection::system()?;
        let manager = ManagerProxy::new(&connection)?;
        let watch = Signals::new()
            .rule(
                Rule::new()
                    .sender(LOGIND)
                    .interface(MANAGER)
                    .path(MANAGER_PATH)
                    .member(PREPARE_FOR_SLEEP),
            )
            .each(&connection, going, move |going| notifier.notify(going))?;
        let held = inhibit(&manager, &inhibitor);
        Ok(Sleeping {
            manager,
            inhibitor,
            held,
            _watch: watch,
        })
    }

    fn handle_request(&mut self, request: Infallible, _events: &EventSink<()>) {
        match request {}
    }

    fn handle_notice(&mut self, going: bool, _events: &EventSink<()>) {
        if going {
            (self.inhibitor.on_sleep)(Sleep::Going(Delay {
                _inhibitor: self.held.take(),
            }));
            return;
        }
        if self.held.is_none() {
            self.held = inhibit(&self.manager, &self.inhibitor);
        }
        (self.inhibitor.on_sleep)(Sleep::Woke);
    }
}

fn going(message: &Message) -> Option<bool> {
    message
        .body()
        .deserialize::<bool>()
        .inspect_err(|error| tracing::debug!("invalid PrepareForSleep signal: {error}"))
        .ok()
}

fn inhibit(manager: &ManagerProxy<'_>, inhibitor: &Inhibitor) -> Option<OwnedFd> {
    manager
        .inhibit(WHAT, &inhibitor.who, &inhibitor.why, MODE)
        .map(OwnedFd::from)
        .inspect_err(|error| tracing::warn!("failed to take sleep inhibitor: {error}"))
        .ok()
}
