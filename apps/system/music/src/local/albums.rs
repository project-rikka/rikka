use std::sync::Arc;

use music_library::{Album, AlbumSort, Albums, GenreId};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::blocks::CARD;
use crate::local::arrange::{Arrange, album_order};
use crate::local::cards::album_card;
use crate::local::empty::empty;
use crate::local::fresh::Shelf;
use crate::local::title_bar;
use crate::music::Music;
use crate::pages::fetched;
use crate::wording::albums;

const SORTS: [AlbumSort; 4] = [
    AlbumSort::Artist,
    AlbumSort::Title,
    AlbumSort::Year,
    AlbumSort::Added,
];
const SORT_WIDTH: f32 = 200.0;

pub(super) fn view(music: &Music, shelf: &Shelf, genre: Option<(GenreId, String)>) -> Element {
    let sort = music.arrange().albums();
    let mut query = Albums::new(sort).order(album_order(sort));
    if let Some((id, _)) = &genre {
        query = query.genre(*id);
    }
    let loading = shelf.read(query, move |library| {
        library.albums(query).map(Arc::<[Album]>::from)
    });
    let title: Label = genre.map_or_else(|| tr!("music.nav.albums"), |(_, name)| Label::from(name));
    let scanning = music.scanning();
    fetched(loading, move |listed: &Arc<[Album]>| {
        if listed.is_empty() {
            return empty(scanning);
        }
        let shown = Arc::clone(listed);
        column! {
            title_bar(
                title.clone(),
                albums(listed.len()),
                sorting(sort),
            );
            lazy_tiles(Length::px(CARD), shown.len(), move |slot| album_card(&shown[slot.index]))
                .gap(Space::SNUG)
                .padding(Inset::edges(Space::NONE, Space::LOOSE, Space::WIDE, Space::LOOSE))
                .grow(1.0)
                .fill_width();
        }
        .fill()
        .any()
    })
}

fn sorting(sort: AlbumSort) -> Element {
    row! {
        icon(MaterialSymbol::SORT).extent(Length::px(18.0)).color(palette().text_muted);
        combo(SORTS.iter().position(|option| *option == sort))
            .options(SORTS.map(named))
            .on_select(|arrange: &mut Arrange, index: usize| {
                if let Some(sort) = SORTS.get(index) {
                    arrange.sort_albums(*sort);
                }
            })
            .width(SORT_WIDTH);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .any()
}

fn named(sort: AlbumSort) -> Label {
    match sort {
        AlbumSort::Artist => tr!("music.sort.artist"),
        AlbumSort::Title => tr!("music.sort.title"),
        AlbumSort::Year => tr!("music.sort.year"),
        AlbumSort::Added => tr!("music.sort.added"),
    }
}
