use futures_util::Stream;

use crate::event::Event;
use crate::id::ItemId;
use crate::item::Item;
use crate::menu::Menu;
use crate::request::{Request, Scroll};
use crate::session::Trays;
use rikka_bus::{Apply, Monitor, Service};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub items: Vec<Item>,
    pub menu: Option<Menu>,
}

impl State {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[must_use]
    pub fn needs_attention(&self) -> bool {
        self.items.iter().any(|item| item.status.needs_attention())
    }

    #[must_use]
    pub fn item(&self, id: &ItemId) -> Option<&Item> {
        self.items.iter().find(|item| &item.id == id)
    }
}

impl Apply<Event> for State {
    fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Changed(items) => self.items.apply(items),
            Event::Opened(menu) => self.menu.apply(Some(*menu)),
            Event::Closed => self.menu.take().is_some(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tray(Monitor<Service<Trays>>);

impl Tray {
    #[must_use]
    pub fn start() -> Tray {
        Tray(Monitor::start())
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state()
    }

    pub fn read<R>(&self, read: impl FnOnce(&State) -> R) -> R {
        self.0.read(read)
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.changes()
    }

    pub fn activate(&self, item: &ItemId, at: (i32, i32)) {
        self.0.request(Request::Activate {
            item: item.clone(),
            at,
        });
    }

    pub fn secondary(&self, item: &ItemId, at: (i32, i32)) {
        self.0.request(Request::Secondary {
            item: item.clone(),
            at,
        });
    }

    pub fn scroll(&self, item: &ItemId, delta: Scroll) {
        self.0.request(Request::Scroll {
            item: item.clone(),
            delta,
        });
    }

    pub fn open_menu(&self, item: &Item, at: (i32, i32)) {
        self.0.request(match item.has_menu() {
            true => Request::Menu(item.id.clone()),
            false => Request::ContextMenu {
                item: item.id.clone(),
                at,
            },
        });
    }

    pub fn choose(&self, item: &ItemId, row: i32) {
        self.0.request(Request::Choose {
            item: item.clone(),
            row,
        });
    }

    pub fn close_menu(&self) {
        let Some(open) = self.read(|tray| tray.menu.as_ref().map(|menu| menu.item.clone())) else {
            return;
        };
        self.0.request(Request::Closed(open));
    }
}
