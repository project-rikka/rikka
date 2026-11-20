use futures_util::Stream;

use crate::service::Watchers;
use crate::user::State;
use rikka_bus::Monitor;

#[derive(Clone, Debug)]
pub struct Privacy(Monitor<Watchers>);

impl Privacy {
    #[must_use]
    pub fn start() -> Privacy {
        Privacy(Monitor::start())
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state()
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.changes()
    }
}
