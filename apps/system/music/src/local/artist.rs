use std::sync::Arc;

use music_core::{Id, Track};
use music_library::{Album, ArtistDetail};
use rikka_app::prelude::*;

use crate::blocks::{Header, Listening};
use crate::local::cards::album_cards;
use crate::local::fresh::Shelf;
use crate::pages::{Block, fetched, page_of};
use crate::wording::{albums, songs};

struct Shown {
    detail: ArtistDetail,
    tracks: Arc<[Track]>,
}

pub(super) fn view(shelf: &Shelf, id: &Id, listening: &Listening) -> Element {
    let Ok(artist_id) = music_local::artist_id(id) else {
        return gone();
    };
    let loading = shelf.read(("artist", artist_id), move |library| {
        let Some(detail) = library.artist(artist_id)? else {
            return Ok(None);
        };
        let tracks = library.artist_tracks(artist_id)?;
        Ok(Some(Shown {
            tracks: music_local::tracks(&tracks).into(),
            detail,
        }))
    });
    let listening = listening.clone();
    fetched(loading, move |shown: &Option<Shown>| match shown {
        Some(shown) => page_of(blocks(shown), listening.clone()),
        None => gone(),
    })
}

fn gone() -> Element {
    status_page(MaterialSymbol::PERSON, tr!("music.local.artist-gone"))
        .fill()
        .any()
}

fn blocks(shown: &Shown) -> Vec<Block> {
    let artist = &shown.detail.artist;
    let mut blocks = vec![Block::Header(Box::new(Header {
        artwork: music_local::artist(artist).artwork,
        round: true,
        kind: tr!("music.kind.artist"),
        title: artist.name.clone(),
        lines: vec![format!(
            "{} · {}",
            albums(artist.albums as usize),
            songs(shown.tracks.len())
        )],
        fidelity: None,
        tracks: Arc::clone(&shown.tracks),
        menu: None,
    }))];
    shelved(
        &mut blocks,
        tr!("music.artist.albums"),
        &shown.detail.albums,
    );
    shelved(
        &mut blocks,
        tr!("music.artist.appears"),
        &shown.detail.appearances,
    );
    blocks
}

fn shelved(blocks: &mut Vec<Block>, title: Label, listed: &[Album]) {
    if listed.is_empty() {
        return;
    }
    blocks.push(Block::Heading(title));
    let listed: Arc<[Album]> = listed.into();
    blocks.push(Block::line(move || album_cards(&listed)));
}
