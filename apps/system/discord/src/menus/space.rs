use discord_core::{ChannelKind, Highlights, NotifyChange, NotifyLevel, Permissions, SpaceId};
use rikka_app::prelude::*;

use super::notify::{level_label, mute_items};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::space_admin::Section;

pub(crate) const APP_DIRECTORY: &str = "https://discord.com/discovery/applications";

pub(super) fn copy_item(label: Label, copied: String) -> MenuItem {
    menu_item(label)
        .icon(MaterialSymbol::TAG)
        .on_press(move || {
            set_text_of(Selection::Clipboard, &copied);
            skip_view();
        })
}

pub(super) fn items(linked: &LinkedAccount, space: &SpaceId) -> Vec<MenuItem> {
    let Some(known) = linked.space(space) else {
        return Vec::new();
    };
    let key = linked.key().to_owned();
    let me = linked.me().map(|me| me.id);
    let owner = me.is_some_and(|me| known.owned_by(&me));
    let notify = known.notify;
    let space = *space;
    let unread = linked.space_unseen(&space).unread;
    let mut items = Vec::new();
    {
        let key = key.clone();
        items.push(
            menu_item(tr!("discord.menu.mark-read"))
                .icon(MaterialSymbol::MARK_CHAT_READ)
                .enabled(unread)
                .on_press(move |chat: &mut Chat| chat.mark_space_read(&key, &space)),
        );
    }
    items.push(menu_separator());
    if known.can(Permissions::CREATE_INSTANT_INVITE) {
        let key = key.clone();
        items.push(
            menu_item(tr!("discord.menu.invite"))
                .icon(MaterialSymbol::PERSON_ADD)
                .on_press(move |chat: &mut Chat| chat.invite_to_space(&key, &space)),
        );
        items.push(menu_separator());
    }
    {
        let key = key.clone();
        items.extend(mute_items(
            known.muted,
            notify.mute_ends,
            tr!("discord.menu.mute-space"),
            tr!("discord.menu.unmute-space"),
            move |chat: &mut Chat, span| chat.mute_space(&key, &space, span),
        ));
    }
    items.push(
        menu_item(tr!("discord.menu.notifications"))
            .icon(MaterialSymbol::NOTIFICATIONS)
            .items(notify_items(&key, space, notify)),
    );
    {
        let key = key.clone();
        let hidden = notify.hide_muted_channels;
        items.push(
            menu_item(tr!("discord.menu.hide-muted"))
                .selected(hidden)
                .on_press(move |chat: &mut Chat| {
                    chat.change_space_notify(
                        &key,
                        &space,
                        NotifyChange::HideMutedChannels(!hidden),
                    );
                }),
        );
    }
    items.push(menu_separator());
    let sections = Section::available(known, owner);
    if !sections.is_empty() {
        let shown = sections.into_iter().map(|section| {
            let key = key.clone();
            menu_item(section.title())
                .on_press(move |chat: &mut Chat| chat.open_space_admin(&key, &space, section))
        });
        items.push(
            menu_item(tr!("discord.menu.space-settings"))
                .icon(MaterialSymbol::SETTINGS)
                .items(shown),
        );
    }
    {
        let key = key.clone();
        items.push(
            menu_item(tr!("discord.menu.privacy"))
                .icon(MaterialSymbol::SHIELD)
                .on_press(move |chat: &mut Chat| chat.open_privacy(&key, &space)),
        );
    }
    if let Some(me) = me.filter(|_| known.can(Permissions::CHANGE_NICKNAME)) {
        let key = key.clone();
        items.push(
            menu_item(tr!("discord.menu.edit-profile"))
                .icon(MaterialSymbol::BADGE)
                .on_press(move |chat: &mut Chat| chat.ask_nickname(&key, &space, &me)),
        );
    }
    if known.can(Permissions::MANAGE_CHANNELS) {
        items.push(menu_separator());
        for (label, kind) in [
            (tr!("discord.menu.create-channel"), ChannelKind::Text),
            (tr!("discord.menu.create-category"), ChannelKind::Category),
        ] {
            let key = key.clone();
            items.push(menu_item(label).icon(MaterialSymbol::ADD_CIRCLE).on_press(
                move |chat: &mut Chat| chat.ask_create_channel(&key, &space, None, kind),
            ));
        }
    }
    items.push(
        menu_item(tr!("discord.menu.app-directory"))
            .icon(MaterialSymbol::OPEN_IN_NEW)
            .on_press(|chat: &mut Chat| chat.open_page(APP_DIRECTORY)),
    );
    if !owner {
        items.push(menu_separator());
        let key = key.clone();
        items.push(
            menu_item(tr!("discord.menu.leave"))
                .icon(MaterialSymbol::LOGOUT)
                .on_press(move |chat: &mut Chat| chat.ask_leave(&key, &space)),
        );
    }
    items.push(menu_separator());
    items.push(copy_item(
        tr!("discord.menu.copy-space-id"),
        space.to_string(),
    ));
    items
}

fn notify_items(key: &str, space: SpaceId, notify: discord_core::SpaceNotify) -> Vec<MenuItem> {
    let change = |label: Label, selected: bool, change: NotifyChange| {
        let key = key.to_owned();
        menu_item(label)
            .selected(selected)
            .on_press(move |chat: &mut Chat| chat.change_space_notify(&key, &space, change))
    };
    let mut items: Vec<MenuItem> = NotifyLevel::ALL
        .into_iter()
        .map(|level| {
            change(
                level_label(level),
                notify.level == level,
                NotifyChange::Level(level),
            )
        })
        .collect();
    items.push(menu_separator());
    items.push(change(
        tr!("discord.menu.suppress-everyone"),
        notify.suppress_everyone,
        NotifyChange::SuppressEveryone(!notify.suppress_everyone),
    ));
    items.push(change(
        tr!("discord.menu.suppress-roles"),
        notify.suppress_roles,
        NotifyChange::SuppressRoles(!notify.suppress_roles),
    ));
    let suppressed = notify.highlights == Highlights::Off;
    items.push(change(
        tr!("discord.menu.suppress-highlights"),
        suppressed,
        NotifyChange::Highlights(match suppressed {
            true => Highlights::On,
            false => Highlights::Off,
        }),
    ));
    items.push(change(
        tr!("discord.menu.mute-events"),
        notify.mute_events,
        NotifyChange::MuteEvents(!notify.mute_events),
    ));
    items.push(menu_separator());
    items.push(change(
        tr!("discord.menu.mobile-push"),
        notify.mobile_push,
        NotifyChange::MobilePush(!notify.mobile_push),
    ));
    items
}

pub(super) fn folder_items(linked: &LinkedAccount, folder: &str) -> Vec<MenuItem> {
    let key = linked.key().to_owned();
    let (read_key, read_folder) = (key.clone(), folder.to_owned());
    let unread = linked.folder(folder).is_some_and(|found| {
        found
            .spaces
            .iter()
            .any(|space| linked.space_unseen(space).unread)
    });
    let edited = folder.to_owned();
    vec![
        menu_item(tr!("discord.menu.mark-folder-read"))
            .icon(MaterialSymbol::MARK_CHAT_READ)
            .enabled(unread)
            .on_press(move |chat: &mut Chat| chat.mark_folder_read(&read_key, &read_folder)),
        menu_item(tr!("discord.menu.folder-settings"))
            .icon(MaterialSymbol::FOLDER)
            .on_press(move |chat: &mut Chat| chat.ask_folder(&key, &edited)),
    ]
}
