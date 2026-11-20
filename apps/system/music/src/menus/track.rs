use std::sync::Arc;

use music_core::Track;
use rikka_app::prelude::*;

use crate::library::{Library, Load};
use crate::menus::rows::Rows;
use crate::menus::{Branch, TrackContext, TrackTarget};
use crate::music::Music;
use crate::nav::Place;
use crate::player::Player;
use crate::wording::service;

pub(super) fn rows(music: &Music, track_target: &TrackTarget) -> Rows {
    let track = &track_target.track;
    let browsed = music.browses(track);
    let mut rows = Rows::default();

    let next = track.clone();
    rows.item_if(
        track.playable,
        tr!("music.menu.play-next"),
        MaterialSymbol::QUEUE_PLAY_NEXT,
        move |player: &mut Player| player.play_next(next.clone()),
    );
    let queued = track.clone();
    rows.item_if(
        track.playable,
        tr!("music.menu.queue"),
        MaterialSymbol::ADD_TO_QUEUE,
        move |player: &mut Player| player.enqueue(std::slice::from_ref(&queued)),
    );
    rows.rule();

    rows.branch(
        tr!("music.menu.add-to-playlist"),
        MaterialSymbol::PLAYLIST_ADD,
        Branch::Playlists,
    );
    match &track_target.context {
        TrackContext::Playlist(playlist) => {
            let (playlist, gone) = (playlist.clone(), track.id.clone());
            rows.item(
                tr!("music.menu.remove-from-playlist"),
                MaterialSymbol::PLAYLIST_REMOVE,
                move |library: &mut Library| library.remove(&playlist, &gone),
            );
        }
        TrackContext::Blend(blend, key) => {
            let (blend, key) = (blend.clone(), *key);
            rows.item(
                tr!("music.menu.remove-from-playlist"),
                MaterialSymbol::PLAYLIST_REMOVE,
                move |music: &mut Music| music.remove_from_blend(&blend, key),
            );
        }
        TrackContext::Upcoming(ahead) => {
            let (ahead, gone) = (*ahead, track.clone());
            rows.item(
                tr!("music.menu.remove-from-queue"),
                MaterialSymbol::REMOVE_FROM_QUEUE,
                move |player: &mut Player| player.drop_upcoming(ahead, &gone),
            );
        }
        TrackContext::Listed | TrackContext::Playing => {}
    }
    if browsed {
        let saved = music.library().is_saved(&track.id);
        let liked = track.clone();
        let (label, symbol) = match saved {
            true => (tr!("music.menu.unsave"), MaterialSymbol::HEART_MINUS),
            false => (tr!("music.menu.save"), MaterialSymbol::FAVORITE),
        };
        rows.item(label, symbol, move |library: &mut Library| {
            library.set_saved(&liked, !saved);
        });
    }
    rows.rule();

    if !browsed {
        let from = service(&track.service);
        rows.quiet(tr!("music.menu.elsewhere", name = from.as_str()));
        rows.rule();
        share(&mut rows, track);
        return rows;
    }
    let seed = track.clone();
    rows.item_if(
        track.playable,
        tr!("music.menu.radio"),
        MaterialSymbol::RADIO,
        move |music: &mut Music| music.go(Place::Mix(seed.clone())),
    );
    let mut credited = track.artists.iter().filter_map(|artist| artist.id.clone());
    match (credited.next(), credited.next()) {
        (Some(artist), None) => rows.item(
            tr!("music.menu.artist"),
            MaterialSymbol::PERSON,
            move |music: &mut Music| music.go(Place::Artist(artist.clone())),
        ),
        (Some(_), Some(_)) => rows.branch(
            tr!("music.menu.artist"),
            MaterialSymbol::PERSON,
            Branch::Artists,
        ),
        (None, _) => {}
    }
    if let Some(album) = track.album.id.clone() {
        rows.item(
            tr!("music.menu.album"),
            MaterialSymbol::ALBUM,
            move |music: &mut Music| music.go(Place::Album(album.clone())),
        );
    }
    rows.rule();

    share(&mut rows, track);
    rows
}

fn share(rows: &mut Rows, track: &Track) {
    let shared = track.clone();
    match track.service.is_local() {
        true => rows.item(
            tr!("music.menu.reveal"),
            MaterialSymbol::FOLDER_OPEN,
            move |music: &mut Music| music.reveal(&shared),
        ),
        false => rows.item(
            tr!("music.menu.copy-song-link"),
            MaterialSymbol::LINK,
            move |music: &mut Music| music.copy_track_link(&shared),
        ),
    }
}

pub(super) fn playlists(music: &Music, track_target: &TrackTarget) -> Rows {
    let mut rows = Rows::default();
    let adding: Arc<[_]> = Arc::from([track_target.track.clone()]);
    let blended = Arc::clone(&adding);
    rows.item(
        tr!("music.menu.new-blend"),
        MaterialSymbol::ADD,
        move |music: &mut Music| music.new_blend(Arc::clone(&blended)),
    );
    let within_blend = match &track_target.context {
        TrackContext::Blend(id, _) => Some(id),
        _ => None,
    };
    for blend in music
        .blends()
        .listed()
        .iter()
        .filter(|blend| Some(blend.id()) != within_blend)
    {
        let (id, adding) = (blend.id().clone(), Arc::clone(&adding));
        rows.item(
            Label::from(blend.name().to_owned()),
            MaterialSymbol::LIBRARY_MUSIC,
            move |music: &mut Music| music.add_to_blend(&id, &adding),
        );
    }
    rows.rule();
    if !music.browses(&track_target.track) || track_target.track.service.is_local() {
        return rows;
    }
    let fresh = Arc::clone(&adding);
    rows.item(
        tr!("music.menu.new-playlist"),
        MaterialSymbol::ADD,
        move |music: &mut Music| music.new_playlist(Arc::clone(&fresh)),
    );
    let library = music.library();
    match library.playlists() {
        Load::Loading => rows.quiet(tr!("music.menu.loading")),
        Load::Failed(why) => rows.quiet(Label::from(why.clone())),
        Load::Ready(_) => {
            let within = match &track_target.context {
                TrackContext::Playlist(id) => Some(id),
                _ => None,
            };
            let service = track_target.track.service.slug;
            for playlist in library
                .editable()
                .into_iter()
                .filter(|playlist| Some(&playlist.id) != within)
                .filter(|playlist| !music.blends().mirrors(service, &playlist.id))
            {
                let adding = Arc::clone(&adding);
                let label = Label::from(playlist.name.clone());
                rows.item(
                    label,
                    MaterialSymbol::QUEUE_MUSIC,
                    move |library: &mut Library| library.add(&playlist, &adding),
                );
            }
        }
    }
    rows
}

pub(super) fn artists(track_target: &TrackTarget) -> Rows {
    let mut rows = Rows::default();
    for artist in &track_target.track.artists {
        let Some(id) = artist.id.clone() else {
            continue;
        };
        rows.item(
            Label::from(artist.name.clone()),
            MaterialSymbol::PERSON,
            move |music: &mut Music| music.go(Place::Artist(id.clone())),
        );
    }
    rows
}
