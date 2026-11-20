use rikka_app::prelude::*;

use crate::menus::{Menus, Subject};
use crate::shell::Shell;
use crate::sidebar::face::Face;
use crate::sidebar::look::{EDGE, PANEL};
use crate::widgets::{DragItem, DropLayout, DropTarget, drop_list, favicon, grip};
use browser_model::{FavoriteId, Row, Slot, Target};

const COLUMNS: usize = 4;
const TILE: f32 = 44.0;
const GAP: f32 = 6.0;
const GLYPH: f32 = 20.0;

pub(crate) fn view(shell: &Shell) -> Element {
    let model = shell.model();
    let favorites: Vec<FavoriteId> = model
        .favorites_here(shell.id())
        .iter()
        .map(|favorite| favorite.id)
        .collect();
    if shell.private() {
        return Element::empty();
    }
    let width = (PANEL - EDGE * 2.0 - GAP * (COLUMNS - 1) as f32) / COLUMNS as f32;
    let tiles = favorites.iter().map(|favorite| {
        component_with(|shell: &Shell| shell, *favorite, tile)
            .width(width)
            .keyed(*favorite)
    });
    drop_list(
        DropLayout::Grid {
            columns: COLUMNS,
            pitch: Size::new(width + GAP, TILE + GAP),
        },
        favorites.len(),
        grid_of(COLUMNS, tiles).gap(GAP).fill_width(),
    )
    .on_drop(|shell: &mut Shell, row: Row, target: DropTarget| {
        shell.dropped(row, Target::Favorites(target.index));
    })
    .min_height(EDGE)
    .fill_width()
    .any()
}

fn tile(shell: &Shell, favorite: &FavoriteId) -> Element {
    let favorite = *favorite;
    let Some(face) = Face::for_slot(&shell.model(), shell.id(), Slot::Favorite(favorite)) else {
        return Element::empty();
    };
    let mut gripped = grip(
        container(crate::memory_saver::dimmed(
            favicon(face.favicon.as_ref(), face.symbol, GLYPH),
            face.discarded,
        ))
        .center()
        .fill(),
    )
    .highlighted(face.lit)
    .on_press(move |shell: &mut Shell| shell.select_slot(Slot::Favorite(favorite)))
    .aim(move |menus: &mut Menus| menus.aim(Subject::Favorite(favorite)))
    .drag_item(DragItem {
        row: Row::Favorite(favorite),
        symbol: face.symbol,
        label: face.title.clone(),
    });
    if let Some(tab) = face.tab {
        gripped = gripped.middle(move |shell: &mut Shell| shell.close(tab));
    }
    gripped.height(TILE).fill_width().any()
}
