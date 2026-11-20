mod album;
mod albums;
mod arrange;
mod artist;
mod artists;
mod cards;
mod empty;
mod folders;
mod fresh;
mod genres;
mod songs;

use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::blocks::Listening;
use crate::music::Music;
use crate::nav::Place;

pub(crate) use arrange::Arrange;
pub(crate) use empty::{meter, scan_line, scanned};
pub(crate) use fresh::Shelf;

pub(crate) fn shelf(music: &Music) -> Option<Shelf> {
    music.local().map(|library| Shelf {
        library: library.clone(),
        indexed: music.indexed(),
        listened: music.listened(),
    })
}

pub(crate) fn page(music: &Music, place: &Place, listening: &Listening) -> Option<Element> {
    if !music.browses_local() {
        return None;
    }
    let shelf = shelf(music)?;
    Some(match place {
        Place::Home => return home(music, &shelf),
        Place::Albums => albums::view(music, &shelf, None),
        Place::Genre { id, name } => albums::view(music, &shelf, Some((*id, name.clone()))),
        Place::Artists => artists::view(music, &shelf),
        Place::Songs => songs::view(music, &shelf, listening),
        Place::Genres => genres::view(music, &shelf),
        Place::Folders => folders::roots(music, &shelf),
        Place::Folder { root, path } => folders::folder(&shelf, root, path, listening),
        Place::Album(id) => album::view(&shelf, id, listening),
        Place::Artist(id) => artist::view(&shelf, id, listening),
        Place::Search
        | Place::Liked
        | Place::Playlist(_)
        | Place::Blend(_)
        | Place::Mix(_)
        | Place::Gallery(_) => return None,
    })
}

fn home(music: &Music, shelf: &Shelf) -> Option<Element> {
    let loading = shelf.read("home", |library| {
        library.recently_added(1).map(|albums| albums.is_empty())
    });
    match loading.ready() {
        Some(true) => Some(empty::empty(music.scanning())),
        Some(false) | None => None,
    }
}

pub(crate) fn title_bar(title: impl Into<Label>, count: Label, trailing: impl View) -> Element {
    row! {
        column! {
            text(title).display().bold().no_wrap();
            text(count).label().muted().no_wrap();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        trailing;
    }
    .gap(Space::NORMAL)
    .align_items(Align::End)
    .padding(Inset::edges(
        Space::WIDE,
        Space::ROOMY,
        Space::ROOMY,
        Space::ROOMY,
    ))
    .fill_width()
    .any()
}
