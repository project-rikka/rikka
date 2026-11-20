use futures_util::Stream;

use crate::action::{Action, Operation, PowerCapabilities};
use crate::event::Event;
use crate::logind::Logind;
use crate::request::Request;
use rikka_bus::{Apply, Monitor, Refused, Refusing, Service};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub capabilities: PowerCapabilities,
    pub brightness: Option<f32>,
    pub error: Option<Refused<Operation>>,
}

impl Refusing for State {
    type Action = Operation;

    fn refused(&mut self) -> &mut Option<Refused<Operation>> {
        &mut self.error
    }
}

impl Apply<Event> for State {
    fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Started {
                capabilities,
                brightness,
            } => {
                let moved = self.capabilities != capabilities || self.brightness != brightness;
                self.capabilities = capabilities;
                self.brightness = brightness;
                moved
            }
            Event::Brightness(brightness) => self.brightness.apply(brightness),
            Event::Answered { operation, error } => self.answer(operation, error),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Power(Monitor<Service<Logind>>);

impl Power {
    #[must_use]
    pub fn start() -> Power {
        Power(Monitor::start())
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state()
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.changes()
    }

    pub fn ask(&self, action: Action) {
        self.0.attempt(Request::Ask(action));
    }

    pub fn brighten(&self, level: f32) {
        self.0.edit(|state| {
            let target = state.brightness.map(|_| level.clamp(0.0, 1.0));
            state.brightness.apply(target)
        });
        self.0.attempt(Request::Brighten(level));
    }

    pub fn reread(&self) {
        self.0.request(Request::Reread);
    }
}
