use music_core::Id;
use music_library::{Album, Artist};
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::artwork::cover;
use crate::blocks::{CARD, UNAVAILABLE, tile};
use crate::music::Music;
use crate::nav::Place;
use crate::widgets::badge;
use crate::wording::{albums, songs};

const AVATAR: f32 = 44.0;
const ARTIST_ROW: f32 = 60.0;

pub(super) fn album_card(album: &Album) -> Element {
    let shown = music_local::album(album);
    let id = shown.id.clone();
    let under = match album.year {
        Some(year) if !album.artist.is_empty() => format!("{} · {year}", album.artist),
        Some(year) => year.to_string(),
        None => album.artist.clone(),
    };
    let card = tile(
        &shown.artwork,
        12.0,
        album.title.clone(),
        under,
        badge(album.fidelity),
        move |music: &mut Music| music.go(Place::Album(id.clone())),
    );
    match album.available {
        true => card,
        false => card.opacity(UNAVAILABLE).any(),
    }
}

pub(super) fn album_cards(shown: &[Album]) -> Element {
    row_of(shown.iter().map(album_card))
        .wrap_items()
        .gap(Space::SNUG)
        .fill_width()
        .any()
}

pub(super) fn artist_card(artist: &Artist) -> Element {
    let shown = music_local::artist(artist);
    let id = shown.id.clone();
    tile(
        &shown.artwork,
        CARD / 2.0,
        artist.name.clone(),
        counted(artist),
        None,
        move |music: &mut Music| music.go(Place::Artist(id.clone())),
    )
}

pub(super) fn artist_row(artist: &Artist) -> Element {
    let shown = music_local::artist(artist);
    let id: Id = shown.id.clone();
    container(
        row! {
            cover(&shown.artwork, AVATAR, AVATAR / 2.0);
            column_of([
                text(artist.name.clone()).label().medium().no_wrap().build(),
                text(counted(artist)).caption().muted().no_wrap().build(),
            ])
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .radius(Corner::Item)
    .on_press(move |music: &mut Music| music.go(Place::Artist(id.clone())))
    .height(ARTIST_ROW)
    .fill_width()
    .any()
}

fn counted(artist: &Artist) -> String {
    format!(
        "{} · {}",
        albums(artist.albums as usize),
        songs(artist.tracks as usize)
    )
}
