use std::time::SystemTime;

use futures_util::Stream;

use crate::session::{Request, Timedated};
use rikka_bus::{Apply, Monitor, Refused, Refusing, Service};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    SetTime,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub zone: Option<String>,
    pub local_rtc: bool,
    pub can_ntp: bool,
    pub ntp: bool,
    pub synchronized: bool,
    pub server: Option<String>,
    pub error: Option<Refused<Action>>,
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Read(State),
    Answered {
        action: Action,
        error: Option<String>,
    },
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
            Event::Read(read) => {
                let next = State {
                    error: self.error.clone(),
                    ..read
                };
                let moved = *self != next;
                *self = next;
                moved
            }
            Event::Answered { action, error } => self.answer(action, error),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TimeDate(Monitor<Service<Timedated>>);

impl TimeDate {
    #[must_use]
    pub fn start() -> TimeDate {
        TimeDate(Monitor::start())
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state()
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.changes()
    }

    pub fn set_time(&self, at: SystemTime) {
        let micros = match at.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(after) => i64::try_from(after.as_micros()).unwrap_or(i64::MAX),
            Err(before) => -i64::try_from(before.duration().as_micros()).unwrap_or(i64::MAX),
        };
        self.0.attempt(Request::SetTime(micros));
    }

    pub fn reread(&self) {
        self.0.request(Request::Reread);
    }
}
