use std::sync::Arc;

use music_library::Genre;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::local::empty::empty;
use crate::local::fresh::Shelf;
use crate::local::title_bar;
use crate::music::Music;
use crate::nav::Place;
use crate::pages::fetched;
use crate::wording::{albums, songs};

const ROW: f32 = 56.0;

pub(super) fn view(music: &Music, shelf: &Shelf) -> Element {
    let scanning = music.scanning();
    let loading = shelf.read("genres", |library| {
        library.genres().map(Arc::<[Genre]>::from)
    });
    fetched(loading, move |listed: &Arc<[Genre]>| {
        if listed.is_empty() {
            return empty(scanning);
        }
        let shown = Arc::clone(listed);
        column! {
            title_bar(
                tr!("music.nav.genres"),
                tr!("music.count.genres", count = &shown.len().to_string()),
                Element::empty(),
            );
            lazy_column(shown.len(), move |slot| genre_row(&shown[slot.index]))
                .estimate(ROW)
                .padding(Inset::edges(Space::NONE, Space::LOOSE, Space::WIDE, Space::LOOSE))
                .grow(1.0)
                .fill_width();
        }
        .fill()
        .any()
    })
}

fn genre_row(genre: &Genre) -> Element {
    let place = Place::Genre {
        id: genre.id,
        name: genre.name.clone(),
    };
    container(
        row! {
            container(icon(MaterialSymbol::SELL).extent(Length::px(20.0)).color(palette().accent))
                .center()
                .background(palette().accent.with_alpha(0.12))
                .radius(Corner::Item)
                .size(Length::px(40.0), Length::px(40.0));
            column! {
                text(genre.name.clone()).label().medium().no_wrap();
                text(format!(
                    "{} · {}",
                    albums(genre.albums as usize),
                    songs(genre.tracks as usize)
                ))
                .caption()
                .muted()
                .no_wrap();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            icon(MaterialSymbol::CHEVRON_RIGHT).extent(Length::px(20.0)).color(palette().text_muted);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .radius(Corner::Item)
    .on_press(move |music: &mut Music| music.go(place.clone()))
    .height(ROW)
    .fill_width()
    .any()
}
