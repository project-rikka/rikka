mod channel;
mod notify;
mod space;

use discord_core::{ChannelId, SpaceId};
use rikka_app::prelude::*;

use crate::chat::Chat;
pub(crate) use notify::level_label;
pub(crate) use space::APP_DIRECTORY;

const WIDTH: f32 = 240.0;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Subject {
    Space(SpaceId),
    Folder(String),
    Channel(ChannelId),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Shown {
    pub(crate) key: String,
    pub(crate) subject: Subject,
    pub(crate) anchor: Anchor,
}

#[derive(Debug, State)]
pub(crate) struct Menus {
    shown: Tracked<Option<Shown>>,
}

impl Menus {
    pub(crate) fn new() -> Menus {
        Menus {
            shown: Tracked::new(None),
        }
    }

    pub(crate) fn shown(&self) -> Option<&Shown> {
        self.shown.as_ref()
    }

    pub(crate) fn show(&mut self, shown: Shown) {
        self.shown.set(Some(shown));
    }

    pub(crate) fn close(&mut self) {
        self.shown.set(None);
    }
}

pub(crate) fn view(chat: &Chat) -> Option<Element> {
    let shown = chat.menus().shown()?.clone();
    let linked = chat.linked(&shown.key)?;
    let items = match &shown.subject {
        Subject::Space(space) => space::items(linked, space),
        Subject::Folder(folder) => space::folder_items(linked, folder),
        Subject::Channel(channel) => channel::items(chat, linked, channel),
    };
    Some(
        menu()
            .items(items)
            .width(WIDTH)
            .on_activate(|menus: &mut Menus, _: usize| menus.close())
            .on_dismiss(|menus: &mut Menus| menus.close())
            .build()
            .keyed(("discord-context-menu", shown.subject.clone()))
            .anchored(shown.anchor),
    )
}
