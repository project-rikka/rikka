use futures_util::StreamExt;
use rikka_tray::{Entry, Item, Scroll, State};
use rikka_types::{Point, PointerButton};
use rikka_ui::{State as UiState, stream};
use shell_apps::AppIcons;

use crate::dropdown_panel::Opening;
use crate::shared::Shared;
use crate::tray::flyout::open::Open;
use crate::tray::look::Look;

#[derive(Debug, UiState)]
pub(crate) struct Flyout {
    shared: Shared,
    tray: State,
    looks: Vec<Look>,
    hovered: Option<usize>,
    open: Option<Open>,
}

impl Flyout {
    #[must_use]
    pub(crate) fn new(shared: &Shared) -> Flyout {
        let mut flyout = Flyout {
            shared: shared.clone(),
            tray: State::default(),
            looks: Vec::new(),
            hovered: None,
            open: None,
        };
        flyout.settle();
        flyout
    }

    pub(crate) fn start(&mut self) {
        stream(
            self.shared.tray.changes().map(|_| ()),
            |flyout: &mut Flyout, ()| flyout.settle(),
        );
    }

    fn settle(&mut self) {
        let next = self.shared.tray.state();
        if next.items != self.tray.items {
            self.looks = next
                .items
                .iter()
                .map(|item| Look::for_item(item, &self.shared.apps))
                .collect();
        }
        self.tray = next;
    }

    #[must_use]
    pub(crate) fn tray(&self) -> &State {
        &self.tray
    }

    #[must_use]
    pub(crate) fn look(&self, row: usize) -> Option<&Look> {
        self.looks.get(row)
    }

    #[must_use]
    pub(crate) fn hovered(&self) -> Option<usize> {
        self.hovered
    }

    #[must_use]
    pub(crate) fn icons(&self) -> &AppIcons {
        self.shared.apps.icons()
    }

    #[must_use]
    pub(crate) fn opening(&self) -> Option<Opening> {
        self.shared.fold.opening()
    }

    #[must_use]
    pub(crate) fn open(&self) -> Option<&Open> {
        self.open.as_ref()
    }

    pub(crate) fn hover(&mut self, row: Option<usize>) {
        self.hovered = row;
    }

    pub(crate) fn dismiss(&mut self) {
        self.shared.tray.close_menu();
        self.shared.fold.fold();
    }

    pub(crate) fn press(&mut self, row: usize, button: PointerButton, at: Point) {
        let Some(item) = self.tray.items.get(row).cloned() else {
            return;
        };
        let at = (at.x as i32, at.y as i32);
        match button {
            PointerButton::Primary if item.presses_open_menu() => self.unfold(&item, row, at),
            PointerButton::Primary => {
                self.shared.tray.activate(&item.id, at);
                self.dismiss();
            }
            PointerButton::Middle => {
                self.shared.tray.secondary(&item.id, at);
                self.dismiss();
            }
            PointerButton::Secondary => self.unfold(&item, row, at),
            PointerButton::Back | PointerButton::Forward | PointerButton::Other(_) => {}
        }
    }

    fn unfold(&mut self, item: &Item, row: usize, at: (i32, i32)) {
        self.shared.tray.open_menu(item, at);
        match item.has_menu() {
            true => self.open = Some(Open::new(item.id.clone(), row)),
            false => self.dismiss(),
        }
    }

    pub(crate) fn scroll(&mut self, row: usize, detents: f32) {
        if let Some(item) = self.tray.items.get(row) {
            self.shared
                .tray
                .scroll(&item.id.clone(), Scroll::Vertical(detents as i32));
        }
    }

    pub(crate) fn highlight(&mut self, level: usize) {
        if let Some(open) = self.open.as_mut() {
            open.fold_under(level);
        }
    }

    pub(super) fn unfold_under(&mut self, level: usize, row: usize, at: f32) {
        if let Some(open) = self.open.as_mut() {
            open.unfold(level, row, at);
        }
    }

    pub(crate) fn chose(&mut self, level: usize, row: usize) {
        let Some((open, menu)) = self.open.as_ref().zip(self.tray.menu.as_ref()) else {
            return;
        };
        let Some(Entry::Row(chosen)) = open.levels(menu).get(level).and_then(|rows| rows.get(row))
        else {
            return;
        };
        if !chosen.enabled || chosen.opens_submenu() {
            return;
        }
        let (item, chosen) = (open.item.clone(), chosen.id);
        self.shared.tray.choose(&item, chosen);
        self.open = None;
        self.shared.fold.fold();
    }

    pub(super) fn close_menu(&mut self) {
        self.shared.tray.close_menu();
        self.open = None;
    }
}
