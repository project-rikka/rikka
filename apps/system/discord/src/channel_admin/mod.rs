mod flow;
mod links;
mod overview;
mod permissions;

use discord_core::{
    ChannelId, ChannelKind, ChannelSettings, OverwriteTarget, SpaceInvite, Webhook,
};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::chat::Chat;
use crate::space_admin::{Edited, Load};

pub(crate) const STAGE: &str = "discord-channel-admin";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Section {
    #[default]
    Overview,
    Permissions,
    Invites,
    Integrations,
    Delete,
}

impl Section {
    fn available(kind: ChannelKind) -> Vec<Section> {
        match kind {
            ChannelKind::Category => vec![Section::Overview, Section::Permissions, Section::Delete],
            _ => vec![
                Section::Overview,
                Section::Permissions,
                Section::Invites,
                Section::Integrations,
                Section::Delete,
            ],
        }
    }

    fn title(self) -> Label {
        match self {
            Section::Overview => tr!("discord.channel-admin.overview"),
            Section::Permissions => tr!("discord.channel-admin.permissions"),
            Section::Invites => tr!("discord.channel-admin.invites"),
            Section::Integrations => tr!("discord.channel-admin.integrations"),
            Section::Delete => tr!("discord.channel-admin.delete"),
        }
    }

    fn nav_item(self) -> NavItem {
        let (symbol, danger) = match self {
            Section::Overview => (MaterialSymbol::INFO, false),
            Section::Permissions => (MaterialSymbol::LOCK, false),
            Section::Invites => (MaterialSymbol::PERSON_ADD, false),
            Section::Integrations => (MaterialSymbol::EXTENSION, false),
            Section::Delete => (MaterialSymbol::DELETE, true),
        };
        let item = nav_item(self.title()).icon(symbol);
        match danger {
            true => item.tint(palette().danger),
            false => item,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Opened {
    pub(crate) key: String,
    pub(crate) channel: ChannelId,
}

#[derive(State)]
pub(crate) struct ChannelAdmin {
    opened: Tracked<Option<Opened>>,
    section: Tracked<Section>,
    warned: Tracked<bool>,
    saving: Tracked<bool>,
    settings: Tracked<Load<Edited<ChannelSettings>>>,
    target: Tracked<Option<OverwriteTarget>>,
    invites: Tracked<Load<Vec<SpaceInvite>>>,
    hooks: Tracked<Load<Vec<Webhook>>>,
    #[untracked]
    name: TextDocument,
    #[untracked]
    topic: TextDocument,
}

impl ChannelAdmin {
    pub(crate) fn new() -> ChannelAdmin {
        ChannelAdmin {
            opened: Tracked::new(None),
            section: Tracked::new(Section::default()),
            warned: Tracked::new(false),
            saving: Tracked::new(false),
            settings: Tracked::new(None),
            target: Tracked::new(None),
            invites: Tracked::new(None),
            hooks: Tracked::new(None),
            name: TextDocument::new(""),
            topic: TextDocument::new(""),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.opened.is_some()
    }

    pub(crate) fn is_dirty(&self) -> bool {
        matches!(self.settings.as_ref(), Some(Ok(edited)) if edited.is_dirty())
    }

    fn draft(&self) -> Option<&ChannelSettings> {
        match self.settings.as_ref()? {
            Ok(edited) => Some(&edited.draft),
            Err(_) => None,
        }
    }

    fn edit(&mut self, change: impl FnOnce(&mut ChannelSettings)) {
        if let Some(Ok(mut edited)) = (*self.settings).clone() {
            change(&mut edited.draft);
            self.settings.set(Some(Ok(edited)));
            self.warned.set(false);
        }
    }
}

impl std::fmt::Debug for ChannelAdmin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChannelAdmin")
            .field("opened", &*self.opened)
            .field("section", &*self.section)
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(chat: &Chat) -> Element {
    let admin = chat.channel_admin();
    let Some(opened) = admin.opened.as_ref().cloned() else {
        return spacer().any();
    };
    let Some(linked) = chat.linked(&opened.key) else {
        return spacer().any();
    };
    let Some(channel) = linked.channel(&opened.channel) else {
        return spacer().any();
    };
    let sections = Section::available(channel.kind);
    let section = *admin.section;
    let at = sections
        .iter()
        .position(|known| *known == section)
        .unwrap_or(0);
    let listed = sections.clone();
    let nav = column! {
        container(text(linked.channel_title(channel)).label().semibold().no_wrap())
            .padding(Inset::xy(Space::SNUG, Space::TIGHT));
        nav_list(at)
            .items(sections.iter().map(|section| section.nav_item()))
            .on_select(move |chat: &mut Chat, index: usize| {
                if let Some(chosen) = listed.get(index) {
                    chat.show_channel_admin(*chosen);
                }
            })
            .grow(1.0);
    }
    .gap(Space::TIGHT)
    .fill();
    let body = match section {
        Section::Overview => overview::view(admin, linked),
        Section::Permissions => permissions::view(admin, linked),
        Section::Invites => links::invites(admin, &opened),
        Section::Integrations => links::hooks(admin),
        Section::Delete => links::delete(linked, &opened.channel),
    };
    let body = column! {
        body.grow(1.0).fill_width();
        if admin.is_dirty() {
            crate::manage::unsaved_bar(
                *admin.warned,
                *admin.saving,
                |chat: &mut Chat| chat.reset_channel_admin(),
                |chat: &mut Chat| chat.save_channel_admin(),
            );
        }
    }
    .gap(Space::NORMAL)
    .fill();
    let split = split_view(nav, body)
        .heading(section.title())
        .shows(at)
        .build()
        .keyed(STAGE);
    let mut layers = vec![split];
    layers.extend(crate::manage::overlays(chat, STAGE));
    column_of(layers).fill().any()
}
