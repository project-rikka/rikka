mod cards;
mod tile;
mod transition_previews;

use cut_project::{TransitionFamily, TransitionKind};
use rikka_app::prelude::*;
use rikka_ui::DragOffer;
use rikka_widgets::column;

use crate::cut::Cut;
use crate::wording;
use tile::Tile;

pub(crate) use transition_previews::TransitionPreviews;

pub(crate) const MIME: &str = "application/x-rikka-cut-transition";
const COLUMNS: usize = 2;

pub(crate) fn gallery(cut: &Cut) -> impl View {
    let ready = !cut.transition_targets().is_empty();
    let transition_previews = cut.transition_previews();
    let sections: Vec<Element> = TransitionFamily::ALL
        .into_iter()
        .map(|family| {
            let tiles: Vec<Element> = family
                .kinds()
                .map(|kind| {
                    Tile {
                        kind,
                        transition_previews: transition_previews.clone(),
                        ready,
                    }
                    .build()
                    .keyed(("cut-transition-tile", kind.index()))
                })
                .collect();
            column! {
                text(wording::transition_family(family)).label().semibold();
                grid_of(COLUMNS, tiles).gap(Space::SNUG).fill_width();
            }
            .gap(Space::SNUG)
            .fill_width()
            .build()
        })
        .collect();
    column_of(sections).gap(Space::ROOMY).fill_width()
}

pub(crate) fn offered(kind: TransitionKind) -> Vec<u8> {
    u8::try_from(kind.index()).map_or_else(|_| Vec::new(), |index| vec![index])
}

pub(crate) fn carried(offer: &DragOffer) -> Option<TransitionKind> {
    let index = *offer.read(MIME)?.first()?;
    TransitionKind::from_index(usize::from(index))
}
