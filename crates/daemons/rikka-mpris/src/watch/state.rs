use futures_util::Stream;

use crate::watch::player::{Control, Player, PlayerId};
use crate::watch::session::{Mpris, Request};
use rikka_bus::{Monitor, Service};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub players: Vec<Player>,
    pub active: Option<PlayerId>,
}

impl State {
    #[must_use]
    pub fn player(&self, id: &PlayerId) -> Option<&Player> {
        self.players.iter().find(|player| &player.id == id)
    }

    #[must_use]
    pub fn active(&self) -> Option<&Player> {
        self.player(self.active.as_ref()?)
    }
}

#[derive(Clone, Debug)]
pub struct Players(Monitor<Service<Mpris>>);

impl Players {
    #[must_use]
    pub fn start() -> Players {
        Players(Monitor::start())
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state()
    }

    #[must_use]
    pub fn active(&self) -> Option<Player> {
        self.0.read(|state| state.active().cloned())
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.changes()
    }

    pub fn control(&self, player: &PlayerId, control: Control) {
        self.0.request(Request {
            player: player.clone(),
            control,
        });
    }

    pub fn control_active(&self, control: Control) {
        if let Some(active) = self.0.read(|state| state.active.clone()) {
            self.control(&active, control);
        }
    }
}
