use std::sync::Arc;

use music_library::Artist;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::blocks::CARD;
use crate::local::arrange::Arrange;
use crate::local::cards::{artist_card, artist_row};
use crate::local::empty::empty;
use crate::local::fresh::Shelf;
use crate::local::title_bar;
use crate::music::Music;
use crate::pages::fetched;

const ROW: f32 = 60.0;

pub(super) fn view(music: &Music, shelf: &Shelf) -> Element {
    let grid = music.arrange().artist_grid();
    let scanning = music.scanning();
    let loading = shelf.read("artists", |library| {
        library.artists().map(Arc::<[Artist]>::from)
    });
    fetched(loading, move |listed: &Arc<[Artist]>| {
        if listed.is_empty() {
            return empty(scanning);
        }
        let shown = Arc::clone(listed);
        let count = shown.len();
        let listing = match grid {
            true => lazy_tiles(Length::px(CARD), count, move |slot| {
                artist_card(&shown[slot.index])
            })
            .gap(Space::SNUG)
            .padding(Inset::edges(
                Space::NONE,
                Space::LOOSE,
                Space::WIDE,
                Space::LOOSE,
            )),
            false => lazy_column(count, move |slot| artist_row(&shown[slot.index]))
                .estimate(ROW)
                .padding(Inset::edges(
                    Space::NONE,
                    Space::LOOSE,
                    Space::WIDE,
                    Space::LOOSE,
                )),
        };
        column! {
            title_bar(
                tr!("music.nav.artists"),
                tr!("music.count.artists", count = &count.to_string()),
                segmented(usize::from(grid))
                    .glyphs([MaterialSymbol::VIEW_LIST, MaterialSymbol::GRID_VIEW])
                    .control_size(ControlSize::Small)
                    .on_select(|arrange: &mut Arrange, index: usize| {
                        arrange.show_artist_grid(index == 1);
                    }),
            );
            listing.grow(1.0).fill_width();
        }
        .fill()
        .any()
    })
}
