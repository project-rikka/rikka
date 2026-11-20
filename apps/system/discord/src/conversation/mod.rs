mod acting;
pub(crate) mod composer;
mod emoji;
mod forum;
mod header;
mod history;
pub(crate) mod lightbox;
mod list;
mod media;
mod message;
mod overlay;
mod sending;
mod ticking;
pub(crate) mod time;
mod widgets;

use std::collections::HashSet;

use discord_core::{ChannelId, ChannelKind};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::account::{LinkedAccount, Scope};
use crate::chat::Chat;

pub(crate) use composer::Composer;
pub(crate) use forum::Forum;
pub(crate) use list::Reading;
pub(crate) use media::Media;
pub(crate) use media::open_link;
pub(crate) use message::rich::rich_view;
pub(crate) use overlay::Overlay;
use sending::Outbox;

pub(crate) type Seat = (String, ChannelId);

const AREA: &str = "discord-conversation";

#[derive(State)]
pub(crate) struct Conversation {
    #[untracked]
    shown: Option<Seat>,
    #[untracked]
    loading: HashSet<Seat>,
    #[untracked]
    outbox: Outbox,
    #[untracked]
    ticks: u32,
    error_message: Tracked<Option<String>>,
    members_shown: Tracked<bool>,
    clock: Revision,
    #[state]
    composer: Composer,
    #[state]
    reading: Reading,
    #[state]
    overlay: Overlay,
    #[state]
    forum: Forum,
    #[state]
    media: Media,
}

impl Conversation {
    pub(crate) fn new() -> Conversation {
        Conversation {
            shown: None,
            loading: HashSet::new(),
            outbox: Outbox::default(),
            ticks: 0,
            error_message: Tracked::new(None),
            members_shown: Tracked::new(true),
            clock: Revision::new(),
            composer: Composer::new(),
            reading: Reading::new(),
            overlay: Overlay::new(),
            forum: Forum::new(),
            media: Media::new(),
        }
    }

    pub(crate) fn composer(&self) -> &TextDocument {
        self.composer.draft()
    }

    pub(crate) fn composing(&self) -> &Composer {
        &self.composer
    }

    pub(crate) fn reading(&self) -> &Reading {
        &self.reading
    }

    pub(crate) fn overlay(&self) -> &Overlay {
        &self.overlay
    }

    pub(crate) fn forum(&self) -> &Forum {
        &self.forum
    }

    pub(crate) fn media(&self) -> &Media {
        &self.media
    }

    pub(crate) fn members_shown(&self) -> bool {
        *self.members_shown
    }

    pub(crate) fn toggle_members(&mut self) {
        let shown = !*self.members_shown;
        self.members_shown.set(shown);
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn dismiss(&mut self) {
        self.error_message.set(None);
    }

    pub(crate) fn complain(&mut self, caption: impl Into<String>) {
        self.error_message.set(Some(caption.into()));
    }

    pub(crate) fn shown(&self) -> Option<&Seat> {
        self.shown.as_ref()
    }

    pub(crate) fn clock(&self) {
        self.clock.read();
    }

    fn show(&mut self, shown: Option<Seat>) -> bool {
        if self.shown == shown {
            return false;
        }
        self.composer.leave(self.shown.as_ref(), shown.as_ref());
        self.overlay.close();
        self.media.stop();
        self.error_message.set(None);
        self.shown = shown;
        true
    }
}

impl std::fmt::Debug for Conversation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Conversation")
            .field("shown", &self.shown)
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(chat: &Chat) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().fill().any();
    };
    let location = linked.location();
    let Some(channel) = location.channel else {
        return empty(linked, &location.scope);
    };
    let Some(channel_info) = linked.channel(&channel) else {
        return empty(linked, &location.scope);
    };
    let body = match channel_info.kind {
        ChannelKind::Forum => component_with(|chat: &Chat| chat, channel, forum::view)
            .grow(1.0)
            .fill_width()
            .any(),
        kind if kind.holds_messages() => column! {
            component_with(|chat: &Chat| chat, channel, list::view).grow(1.0);
            component_with(|chat: &Chat| chat, channel, composer::view);
        }
        .grow(1.0)
        .fill_width()
        .any(),
        _ => return empty(linked, &location.scope),
    };
    column_of([
        column! {
            component_with(|chat: &Chat| chat, channel, header::view);
            body;
        }
        .fill()
        .build()
        .keyed(AREA),
        component(|chat: &Chat| chat, overlay::view),
    ])
    .fill()
    .any()
}

fn empty(linked: &LinkedAccount, scope: &Scope) -> Element {
    let (symbol, caption) = match scope {
        Scope::Home => (MaterialSymbol::GROUP, tr!("discord.conversation.home")),
        Scope::Space(space) => (
            MaterialSymbol::TAG,
            match linked.space(space) {
                Some(space_info) => tr!(
                    "discord.conversation.no-channel",
                    name = space_info.name.as_str()
                ),
                None => tr!("discord.conversation.unknown-space"),
            },
        ),
    };
    status_page(symbol, caption).fill().any()
}
