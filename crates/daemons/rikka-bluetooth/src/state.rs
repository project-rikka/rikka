use futures_util::Stream;

use crate::adapter::{Adapter, AdapterId};
use crate::device::{Device, DeviceId};
use crate::event::Event;
use crate::prompt::Reply;
use crate::request::{Action, Request};
use crate::service::Session;
use crate::session::Agency;
use crate::snapshot::Snapshot;
use rikka_bus::{Apply, Monitor, Refused, Refusing};

#[derive(Clone, Debug, Default)]
pub struct State {
    pub snapshot: Snapshot,
    pub error: Option<Refused<Action>>,
}

impl State {
    #[must_use]
    pub fn radio(&self) -> Option<&Adapter> {
        self.snapshot.adapters.first()
    }

    #[must_use]
    pub fn is_on(&self) -> bool {
        self.radio().is_some_and(|adapter| adapter.powered)
    }

    #[must_use]
    pub fn known(&self) -> Vec<&Device> {
        self.radio()
            .map(|adapter| self.snapshot.known(&adapter.id))
            .unwrap_or_default()
    }
}

impl Refusing for State {
    type Action = Action;

    fn refused(&mut self) -> &mut Option<Refused<Action>> {
        &mut self.error
    }
}

impl Apply<Event> for State {
    fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Changed(snapshot) => self.snapshot.apply(*snapshot),
            Event::Answered { action, error, .. } => self.answer(action, error),
            Event::Asked(_) | Event::Withdrew(_) => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Bluetooth(Monitor<Session>);

impl Bluetooth {
    #[must_use]
    pub fn start() -> Bluetooth {
        Bluetooth(Monitor::start())
    }

    #[must_use]
    pub fn answering() -> Bluetooth {
        Bluetooth(Monitor::start_with(|report, unavailable| {
            Session::start(report, unavailable, Agency::Answers)
        }))
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state()
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.changes()
    }

    pub fn events(&self) -> impl Stream<Item = Event> + Send + 'static {
        self.0.events()
    }

    #[must_use]
    pub fn unavailable(&self) -> Option<String> {
        self.0.unavailable()
    }

    pub fn unavailable_changes(&self) -> impl Stream<Item = Option<String>> + Send + 'static {
        self.0.unavailable_changes()
    }

    pub fn answer(&self, prompt: u64, reply: Reply) {
        if let Some(session) = self.0.backend() {
            session.answer(prompt, reply);
        }
    }

    pub fn use_radio(&self, on: bool) {
        if let Some(adapter) = self.radio() {
            self.send(Request::Power(adapter, on));
        }
    }

    fn radio(&self) -> Option<AdapterId> {
        self.0
            .read(|state| state.radio().map(|adapter| adapter.id.clone()))
    }

    pub fn connect(&self, device: &DeviceId, connected: bool) {
        self.send(match connected {
            true => Request::Connect(device.clone()),
            false => Request::Disconnect(device.clone()),
        });
    }

    pub fn send(&self, request: Request) {
        self.0.attempt(request);
    }
}
