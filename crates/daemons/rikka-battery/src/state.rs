use std::time::Duration;

use futures_util::{Stream, StreamExt};

use crate::device::{Device, DeviceId};
use crate::history::Sample;
use crate::pack::Pack;
use crate::reading::Reading;
use crate::session::{Request, Upower};
use rikka_bus::{Apply, Monitor, Refused, Refusing, Service};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Preserve,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub main: Option<Reading>,
    pub on_battery: bool,
    pub devices: Vec<Device>,
    pub packs: Vec<Pack>,
    pub history: Vec<Sample>,
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
pub struct Battery(Monitor<Service<Upower>>);

impl Battery {
    #[must_use]
    pub fn start() -> Battery {
        Battery(Monitor::start())
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state()
    }

    #[must_use]
    pub fn main(&self) -> Option<Reading> {
        self.0.read(|state| state.main)
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.changes()
    }

    pub fn read_history(&self, span: Duration, points: u32) {
        self.0.request(Request::History { span, points });
    }

    pub fn main_changes(&self) -> impl Stream<Item = Option<Reading>> + Send + 'static {
        let mut main = self.main();
        self.0.changes().filter_map(move |state| {
            let moved = state.main != main;
            main = state.main;
            std::future::ready(moved.then_some(state.main))
        })
    }

    pub fn preserve(&self, pack: &DeviceId, on: bool) {
        let id = pack.clone();
        self.0.edit(move |state| {
            let Some(threshold) = state
                .packs
                .iter_mut()
                .find(|known| known.id == id)
                .and_then(|known| known.threshold.as_mut())
            else {
                return false;
            };
            let moved = threshold.enabled != on;
            threshold.enabled = on;
            moved
        });
        self.0.attempt(Request::Preserve {
            pack: pack.clone(),
            on,
        });
    }
}
