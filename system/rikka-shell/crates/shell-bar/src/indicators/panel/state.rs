use rikka_ui::{State, stream};

use crate::indicators::Shelf;

#[derive(Debug, State)]
pub(crate) struct Drawer {
    shelf: Shelf,
}

impl Drawer {
    #[must_use]
    pub(crate) fn new(shelf: &Shelf) -> Drawer {
        Drawer {
            shelf: shelf.clone(),
        }
    }

    pub(crate) fn start(&mut self) {
        stream(self.shelf.changes(), moved);
    }

    #[must_use]
    pub(crate) fn shelf(&self) -> &Shelf {
        &self.shelf
    }

    pub(crate) fn dismiss(&mut self) {
        self.shelf.close();
    }
}

fn moved(_: &mut Drawer, (): ()) {}
