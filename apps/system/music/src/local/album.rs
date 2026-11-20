use std::sync::Arc;

use music_core::{Id, Track};
use music_library::AlbumDetail;
use rikka_app::prelude::*;

use crate::blocks::{Header, Leading, Listening};
use crate::local::fresh::Shelf;
use crate::pages::{Block, fetched, page_of};
use crate::wording::{self, songs, span};

struct Shown {
    detail: AlbumDetail,
    tracks: Arc<[Track]>,
}

pub(super) fn view(shelf: &Shelf, id: &Id, listening: &Listening) -> Element {
    let Ok(album_id) = music_local::album_id(id) else {
        return gone();
    };
    let loading = shelf.read(("album", album_id), move |library| {
        Ok(library.album(album_id)?.map(|detail| Shown {
            tracks: music_local::tracks(&detail.tracks).into(),
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
    status_page(MaterialSymbol::ALBUM, tr!("music.local.album-gone"))
        .fill()
        .any()
}

fn blocks(shown: &Shown) -> Vec<Block> {
    let album = &shown.detail.album;
    let core = music_local::album(album);
    let mut lines = vec![
        album.artist.clone(),
        format!(
            "{} · {} · {}",
            wording::release(&core),
            songs(album.tracks as usize),
            span(album.duration)
        ),
    ];
    if !album.available {
        lines.push(tr!("music.local.offline").to_string());
    }
    let mut blocks = vec![
        Block::Header(Box::new(Header {
            artwork: core.artwork.clone(),
            round: false,
            kind: tr!("music.kind.album"),
            title: album.title.clone(),
            lines,
            fidelity: Some(album.fidelity),
            tracks: Arc::clone(&shown.tracks),
            menu: None,
        })),
        Block::Columns,
    ];
    let rows = &shown.detail.tracks;
    let discs = album.discs > 1;
    let mut start = 0;
    while start < rows.len() {
        let disc = rows[start].disc;
        let end = rows[start..]
            .iter()
            .position(|row| row.disc != disc)
            .map_or(rows.len(), |length| start + length);
        if discs {
            let named = disc_name(disc, rows[start].disc_subtitle.as_deref());
            blocks.push(Block::Heading(named));
        }
        blocks.push(Block::Tracks {
            list: Arc::clone(&shown.tracks),
            span: start..end,
            leading: Leading::Number,
            within: None,
        });
        start = end;
    }
    blocks
}

fn disc_name(disc: Option<u32>, subtitle: Option<&str>) -> Label {
    let number = disc.unwrap_or(1).to_string();
    match subtitle.filter(|subtitle| !subtitle.is_empty()) {
        Some(subtitle) => tr!("music.local.disc-named", number = &number, name = subtitle),
        None => tr!("music.local.disc", number = &number),
    }
}
