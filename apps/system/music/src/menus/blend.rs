use music_blend::BlendId;
use rikka_app::prelude::*;

use crate::dialogs::{Dialog, Dialogs, Named};
use crate::menus::rows::Rows;
use crate::music::{Music, Start};

pub(super) fn rows(music: &Music, id: &BlendId) -> Rows {
    let mut rows = Rows::default();
    let Some(blend) = music.blends().find(id) else {
        return rows;
    };

    let played = id.clone();
    rows.item_if(
        !blend.is_empty(),
        tr!("music.play"),
        MaterialSymbol::PLAY_ARROW,
        move |music: &mut Music| music.start_blend(&played, Start::Play),
    );
    let queued = id.clone();
    rows.item_if(
        !blend.is_empty(),
        tr!("music.menu.queue"),
        MaterialSymbol::ADD_TO_QUEUE,
        move |music: &mut Music| music.start_blend(&queued, Start::Queue),
    );
    rows.rule();

    let synced = id.clone();
    rows.item_if(
        music.has_links() && !music.blends().is_syncing(id),
        tr!("music.menu.sync"),
        MaterialSymbol::SYNC,
        move |music: &mut Music| music.sync_blend(&synced),
    );
    let named = Named::Blend {
        id: id.clone(),
        name: blend.name().to_owned(),
    };
    let renamed = named.clone();
    rows.item(
        tr!("music.menu.rename"),
        MaterialSymbol::EDIT,
        move |dialogs: &mut Dialogs| dialogs.ask(Dialog::Rename(renamed.clone())),
    );
    rows.item(
        tr!("music.menu.delete"),
        MaterialSymbol::DELETE,
        move |dialogs: &mut Dialogs| dialogs.ask(Dialog::Delete(named.clone())),
    );
    rows
}
