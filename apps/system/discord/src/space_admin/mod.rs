mod audit;
mod automod;
mod bans;
mod community;
mod docs;
mod expressions;
mod flow;
mod hooks;
mod invites;
mod members;
mod onboarding;
mod overview;
mod roles;
mod safety;
mod section;
mod upload;
mod widgets;

use std::collections::HashMap;

use discord_core::{
    AuditEntry, AuditQuery, AutomodRule, Ban, Integration, MemberPage, Onboarding, RoleId,
    SpaceEmoji, SpaceId, SpaceInvite, SpaceSettings, SpaceSound, SpaceSticker, Template, UserId,
    Vanity, Webhook, WelcomeScreen, Widget,
};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::account::LinkedAccount;
use crate::chat::Chat;
pub(crate) use docs::Docs;
pub(crate) use invites::invite_row;
pub(crate) use roles::RoleDraft;
use roles::RoleTab;
pub(crate) use section::Section;
pub(crate) use upload::Upload;

pub(crate) const STAGE: &str = "discord-space-admin";
const PICKS_MOST: f32 = 180.0;

pub(crate) type Load<T> = Option<Result<T, String>>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Edited<T> {
    pub(crate) original: T,
    pub(crate) draft: T,
}

impl<T: Clone + PartialEq> Edited<T> {
    pub(crate) fn new(value: T) -> Edited<T> {
        Edited {
            original: value.clone(),
            draft: value,
        }
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.original != self.draft
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Opened {
    pub(crate) key: String,
    pub(crate) space: SpaceId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Images {
    pub(crate) icon: Option<Option<discord_core::ImageUpload>>,
    pub(crate) banner: Option<Option<discord_core::ImageUpload>>,
    pub(crate) splash: Option<Option<discord_core::ImageUpload>>,
}

impl Images {
    fn is_empty(&self) -> bool {
        self.icon.is_none() && self.banner.is_none() && self.splash.is_none()
    }
}

#[derive(State)]
pub(crate) struct SpaceAdmin {
    opened: Tracked<Option<Opened>>,
    section: Tracked<Section>,
    warned: Tracked<bool>,
    saving: Tracked<bool>,
    settings: Tracked<Load<Edited<SpaceSettings>>>,
    images: Tracked<Images>,
    role: Tracked<Option<Edited<RoleDraft>>>,
    role_tab: Tracked<RoleTab>,
    role_members: Tracked<Load<Vec<UserId>>>,
    role_counts: Tracked<Load<HashMap<RoleId, u32>>>,
    emojis: Tracked<Load<Vec<SpaceEmoji>>>,
    stickers: Tracked<Load<Vec<SpaceSticker>>>,
    sounds: Tracked<Load<Vec<SpaceSound>>>,
    hooks: Tracked<Load<Vec<Webhook>>>,
    integrations: Tracked<Load<Vec<Integration>>>,
    invites: Tracked<Load<Vec<SpaceInvite>>>,
    bans: Tracked<Load<Vec<Ban>>>,
    ban_filter: Tracked<String>,
    audit: Tracked<Load<Vec<AuditEntry>>>,
    audit_query: Tracked<AuditQuery>,
    audit_end: Tracked<bool>,
    templates: Tracked<Load<Vec<Template>>>,
    rules: Tracked<Load<Vec<AutomodRule>>>,
    rule: Tracked<Option<AutomodRule>>,
    members: Tracked<Load<MemberPage>>,
    #[untracked]
    member_query: String,
    widget: Tracked<Load<Edited<Widget>>>,
    vanity: Tracked<Load<Edited<Vanity>>>,
    welcome: Tracked<Load<Edited<WelcomeScreen>>>,
    onboarding: Tracked<Load<Edited<Onboarding>>>,
    prompt_at: Tracked<Option<usize>>,
    upload: Tracked<Option<Upload>>,
    renaming: Tracked<Option<u64>>,
    hook: Tracked<Option<Webhook>>,
    community_rules: Tracked<Option<discord_core::ChannelId>>,
    community_updates: Tracked<Option<discord_core::ChannelId>>,
    #[untracked]
    docs: Docs,
}

impl SpaceAdmin {
    pub(crate) fn new() -> SpaceAdmin {
        SpaceAdmin {
            opened: Tracked::new(None),
            section: Tracked::new(Section::default()),
            warned: Tracked::new(false),
            saving: Tracked::new(false),
            settings: Tracked::new(None),
            images: Tracked::new(Images::default()),
            role: Tracked::new(None),
            role_tab: Tracked::new(RoleTab::default()),
            role_members: Tracked::new(None),
            role_counts: Tracked::new(None),
            emojis: Tracked::new(None),
            stickers: Tracked::new(None),
            sounds: Tracked::new(None),
            hooks: Tracked::new(None),
            integrations: Tracked::new(None),
            invites: Tracked::new(None),
            bans: Tracked::new(None),
            ban_filter: Tracked::new(String::new()),
            audit: Tracked::new(None),
            audit_query: Tracked::new(AuditQuery::default()),
            audit_end: Tracked::new(false),
            templates: Tracked::new(None),
            rules: Tracked::new(None),
            rule: Tracked::new(None),
            members: Tracked::new(None),
            member_query: String::new(),
            widget: Tracked::new(None),
            vanity: Tracked::new(None),
            welcome: Tracked::new(None),
            onboarding: Tracked::new(None),
            prompt_at: Tracked::new(None),
            upload: Tracked::new(None),
            renaming: Tracked::new(None),
            hook: Tracked::new(None),
            community_rules: Tracked::new(None),
            community_updates: Tracked::new(None),
            docs: Docs::default(),
        }
    }

    pub(crate) fn opened(&self) -> Option<&Opened> {
        self.opened.as_ref()
    }

    pub(crate) fn is_open(&self) -> bool {
        self.opened.is_some()
    }

    pub(crate) fn section(&self) -> Section {
        *self.section
    }

    pub(crate) fn is_dirty(&self) -> bool {
        match *self.section {
            Section::Overview | Section::Safety | Section::Community => {
                !self.images.is_empty() || loaded_dirty(&self.settings)
            }
            Section::Roles => self.role.as_ref().is_some_and(Edited::is_dirty),
            Section::Widget => loaded_dirty(&self.widget),
            Section::Vanity => loaded_dirty(&self.vanity),
            Section::Welcome => loaded_dirty(&self.welcome),
            Section::Onboarding => loaded_dirty(&self.onboarding),
            _ => false,
        }
    }

    fn settings_original(&self) -> Option<&SpaceSettings> {
        match self.settings.as_ref()? {
            Ok(edited) => Some(&edited.original),
            Err(_) => None,
        }
    }

    pub(crate) fn edit_settings(&mut self, change: impl FnOnce(&mut SpaceSettings)) {
        if let Some(Ok(mut edited)) = (*self.settings).clone() {
            change(&mut edited.draft);
            self.settings.set(Some(Ok(edited)));
            self.warned.set(false);
        }
    }

    pub(crate) fn close(&mut self) {
        *self = SpaceAdmin::new();
    }
}

fn loaded_dirty<T: Clone + PartialEq>(load: &Load<Edited<T>>) -> bool {
    matches!(load, Some(Ok(edited)) if edited.is_dirty())
}

impl std::fmt::Debug for SpaceAdmin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SpaceAdmin")
            .field("opened", &*self.opened)
            .field("section", &*self.section)
            .finish_non_exhaustive()
    }
}

pub(crate) fn loading<T>(load: &Load<T>, shown: impl FnOnce(&T) -> Element) -> Element {
    match load {
        None => container(spinner())
            .center()
            .fill_width()
            .padding(Inset::all(Space::LOOSE))
            .any(),
        Some(Err(error_message)) => text(error_message.clone())
            .caption()
            .danger()
            .fill_width()
            .any(),
        Some(Ok(value)) => shown(value),
    }
}

pub(crate) fn toggle<T: PartialEq + Copy>(listed: &mut Vec<T>, item: T, on: bool) {
    listed.retain(|known| *known != item);
    if on {
        listed.push(item);
    }
}

pub(crate) fn picks<T: Copy + PartialEq + 'static>(
    choices: Vec<(T, String)>,
    chosen: &[T],
    set: impl Fn(&mut SpaceAdmin, T, bool) + Clone + 'static,
) -> Element {
    scrollable(
        column_of(choices.into_iter().map(|(id, name)| {
            let set = set.clone();
            checkbox(chosen.contains(&id))
                .label(text(name).label())
                .on_toggle(move |admin: &mut SpaceAdmin, on: bool| set(admin, id, on))
                .any()
        }))
        .gap(Space::HAIR)
        .fill_width(),
    )
    .max_height(PICKS_MOST)
    .fill_width()
    .any()
}

pub(crate) fn hint(label: Label) -> Element {
    text(label).caption().muted().fill_width().any()
}

pub(crate) fn view(chat: &Chat) -> Element {
    let admin = chat.space_admin();
    let Some(opened) = admin.opened().cloned() else {
        return spacer().any();
    };
    let Some(linked) = chat.linked(&opened.key) else {
        return spacer().any();
    };
    let Some(space) = linked.space(&opened.space) else {
        return spacer().any();
    };
    let owner = linked.me().is_some_and(|me| space.owned_by(&me.id));
    let sections = Section::available(space, owner);
    let section = admin.section();
    let mut entries: Vec<NavItem> = Vec::new();
    let mut indices: Vec<Option<Section>> = Vec::new();
    let mut group = None;
    for listed in &sections {
        if group != Some(listed.group()) {
            group = Some(listed.group());
            entries.push(nav_header(listed.group().title()));
            indices.push(None);
        }
        entries.push(listed.nav_item());
        indices.push(Some(*listed));
    }
    let at = indices
        .iter()
        .position(|entry| *entry == Some(section))
        .unwrap_or(0);
    let nav = column! {
        container(text(space.name.clone()).label().semibold().no_wrap())
            .padding(Inset::xy(Space::SNUG, Space::TIGHT));
        nav_list(at)
            .items(entries)
            .on_select(move |chat: &mut Chat, index: usize| {
                if let Some(Some(chosen)) = indices.get(index) {
                    chat.show_space_admin(*chosen);
                }
            })
            .grow(1.0);
    }
    .gap(Space::TIGHT)
    .fill();
    let body = column! {
        body(chat, linked, &opened, section).grow(1.0).fill_width();
        if admin.is_dirty() {
            crate::manage::unsaved_bar(
                admin.is_warned(),
                admin.is_saving(),
                |chat: &mut Chat| chat.reset_space_admin(),
                |chat: &mut Chat| chat.save_space_admin(),
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

fn body(chat: &Chat, linked: &LinkedAccount, opened: &Opened, section: Section) -> Element {
    let admin = chat.space_admin();
    let space = &opened.space;
    match section {
        Section::Overview => overview::view(admin, linked, space),
        Section::Roles => roles::view(admin, linked, space),
        Section::Emoji => expressions::emoji(admin, linked, space),
        Section::Stickers => expressions::stickers(admin, linked, space),
        Section::Soundboard => expressions::sounds(admin, linked, space),
        Section::Widget => widgets::widget(admin, linked, space),
        Section::Template => widgets::template(admin),
        Section::Vanity => widgets::vanity(admin, linked, space),
        Section::Integrations => hooks::view(admin, linked, space),
        Section::Safety => safety::view(admin, linked, space),
        Section::AutoMod => automod::view(admin, linked, space),
        Section::Community => community::view(admin, linked, space),
        Section::Onboarding => onboarding::view(admin, linked, space),
        Section::Welcome => community::welcome(admin, linked, space),
        Section::Members => members::view(admin, linked, space),
        Section::Invites => invites::view(admin, linked, space),
        Section::Bans => bans::view(admin),
        Section::AuditLog => audit::view(admin, linked, space),
        Section::Delete => safety::delete(linked, space),
        Section::Boost | Section::Subscriptions | Section::Insights => spacer().any(),
    }
}
