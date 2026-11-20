use futures_util::Stream;

use crate::profile::{Degraded, Hold, Profile};
use crate::session::{Daemon, Request};
use rikka_bus::{Apply, Monitor, Refused, Refusing, Service};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub active: Option<Profile>,
    pub profiles: Vec<Profile>,
    pub degraded: Option<Degraded>,
    pub holds: Vec<Hold>,
    pub error: Option<Refused<Profile>>,
}

impl State {
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.active.is_some()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Reading {
    pub(crate) active: Option<Profile>,
    pub(crate) profiles: Vec<Profile>,
    pub(crate) degraded: Option<Degraded>,
    pub(crate) holds: Vec<Hold>,
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Read(Reading),
    Answered {
        profile: Profile,
        error: Option<String>,
    },
}

impl Refusing for State {
    type Action = Profile;

    fn refused(&mut self) -> &mut Option<Refused<Profile>> {
        &mut self.error
    }
}

impl Apply<Event> for State {
    fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Read(reading) => {
                let moved = self.active != reading.active
                    || self.profiles != reading.profiles
                    || self.degraded != reading.degraded
                    || self.holds != reading.holds;
                self.active = reading.active;
                self.profiles = reading.profiles;
                self.degraded = reading.degraded;
                self.holds = reading.holds;
                moved
            }
            Event::Answered { profile, error } => self.answer(profile, error),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PowerProfiles(Monitor<Service<Daemon>>);

impl PowerProfiles {
    #[must_use]
    pub fn start() -> PowerProfiles {
        PowerProfiles(Monitor::start())
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state()
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.changes()
    }

    pub fn set(&self, profile: Profile) {
        self.0.edit(|state| {
            if !state.profiles.contains(&profile) {
                return false;
            }
            state.active.apply(Some(profile))
        });
        self.0.attempt(Request::Set(profile));
    }
}
