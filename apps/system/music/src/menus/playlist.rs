use music_core::{Playlist, Shared};
use rikka_app::prelude::*;

use crate::dialogs::{Dialog, Dialogs, Named};
use crate::menus::rows::Rows;
use crate::music::{Music, Start};

pub(super) fn rows(music: &Music, playlist: &Playlist) -> Rows {
    let library = music.library();
    let listed = library.playlist(&playlist.id).cloned();
    let known = Playlist {
        editable: library.can_edit(playlist),
        ..listed.clone().unwrap_or_else(|| playlist.clone())
    };
    let mut rows = Rows::default();

    let played = known.id.clone();
    rows.item(
        tr!("music.play"),
        MaterialSymbol::PLAY_ARROW,
        move |music: &mut Music| music.start_playlist(&played, Start::Play),
    );
    let queued = known.id.clone();
    rows.item(
        tr!("music.menu.queue"),
        MaterialSymbol::ADD_TO_QUEUE,
        move |music: &mut Music| music.start_playlist(&queued, Start::Queue),
    );
    rows.rule();

    if known.editable {
        let renamed = known.clone();
        rows.item(
            tr!("music.menu.rename"),
            MaterialSymbol::EDIT,
            move |dialogs: &mut Dialogs| {
                dialogs.ask(Dialog::Rename(Named::Playlist(renamed.clone())))
            },
        );
        let deleted = known.clone();
        rows.item(
            tr!("music.menu.delete"),
            MaterialSymbol::DELETE,
            move |dialogs: &mut Dialogs| {
                dialogs.ask(Dialog::Delete(Named::Playlist(deleted.clone())))
            },
        );
    } else if let Some(listed) = listed {
        rows.item(
            tr!("music.menu.forget"),
            MaterialSymbol::BOOKMARK_REMOVE,
            move |music: &mut Music| music.delete_playlist(&listed),
        );
    }
    rows.rule();

    let shared = known.id.clone();
    rows.item(
        tr!("music.menu.copy-playlist-link"),
        MaterialSymbol::LINK,
        move |music: &mut Music| music.copy_link(Shared::Playlist(&shared)),
    );
    rows
}
