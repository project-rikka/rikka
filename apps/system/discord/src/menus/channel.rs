use discord_client::writing;
use discord_core::{Channel, ChannelId, ChannelKind, NotifyLevel, Permissions};
use rikka_app::prelude::*;

use super::notify::{level_label, mute_items};
use super::space::copy_item;
use crate::account::LinkedAccount;
use crate::chat::Chat;

pub(super) fn items(chat: &Chat, linked: &LinkedAccount, channel: &ChannelId) -> Vec<MenuItem> {
    let Some(known) = linked.channel(channel) else {
        return Vec::new();
    };
    let Some(space) = known.space else {
        return Vec::new();
    };
    let key = linked.key().to_owned();
    let channel = *channel;
    let category = known.kind == ChannelKind::Category;
    let can = |permission: Permissions| known.permissions.contains(permission);
    let mut items = Vec::new();
    {
        let key = key.clone();
        let unread = match category {
            true => linked
                .sections(&space)
                .iter()
                .filter(|section| section.category.is_some_and(|parent| parent.id == channel))
                .flat_map(|section| section.rows.iter())
                .any(|row| linked.unseen(&row.channel.id).unread),
            false => linked.unseen(&channel).unread,
        };
        items.push(
            menu_item(tr!("discord.menu.mark-read"))
                .icon(MaterialSymbol::MARK_CHAT_READ)
                .enabled(unread)
                .on_press(move |chat: &mut Chat| chat.mark_beneath_read(&key, &channel)),
        );
    }
    items.push(menu_separator());
    if category {
        let collapsed = chat.collapsed_categories().contains(&channel);
        items.push(
            menu_item(tr!("discord.menu.collapse-category"))
                .selected(collapsed)
                .on_press(move |chat: &mut Chat| chat.toggle_category(&channel)),
        );
        items.push(
            menu_item(tr!("discord.menu.collapse-all"))
                .on_press(move |chat: &mut Chat| chat.collapse_all_categories(&space)),
        );
    } else {
        if can(Permissions::CREATE_INSTANT_INVITE) && known.kind.holds_messages() {
            let key = key.clone();
            items.push(
                menu_item(tr!("discord.menu.invite"))
                    .icon(MaterialSymbol::PERSON_ADD)
                    .on_press(move |chat: &mut Chat| chat.open_invite(&key, &channel)),
            );
        }
        let link = writing::channel_link(known);
        items.push(
            menu_item(tr!("discord.menu.copy-link"))
                .icon(MaterialSymbol::LINK)
                .on_press(move || {
                    set_text_of(Selection::Clipboard, &link);
                    skip_view();
                }),
        );
    }
    items.push(menu_separator());
    {
        let key = key.clone();
        let (mute, unmute) = match category {
            true => (
                tr!("discord.menu.mute-category"),
                tr!("discord.menu.unmute-category"),
            ),
            false => (
                tr!("discord.menu.mute-channel"),
                tr!("discord.menu.unmute-channel"),
            ),
        };
        items.extend(mute_items(
            known.muted,
            known.notify.mute_ends,
            mute,
            unmute,
            move |chat: &mut Chat, span| chat.mute_channel(&key, &space, &channel, span),
        ));
    }
    items.push(
        menu_item(tr!("discord.menu.notifications"))
            .icon(MaterialSymbol::NOTIFICATIONS)
            .items(level_items(&key, known)),
    );
    let manages = can(Permissions::MANAGE_CHANNELS);
    if manages || can(Permissions::MANAGE_ROLES) {
        items.push(menu_separator());
        let key = key.clone();
        items.push(
            menu_item(match category {
                true => tr!("discord.menu.edit-category"),
                false => tr!("discord.menu.edit-channel"),
            })
            .icon(MaterialSymbol::SETTINGS)
            .on_press(move |chat: &mut Chat| chat.open_channel_admin(&key, &channel)),
        );
    }
    if manages {
        if category {
            let key = key.clone();
            items.push(
                menu_item(tr!("discord.menu.create-channel"))
                    .icon(MaterialSymbol::ADD_CIRCLE)
                    .on_press(move |chat: &mut Chat| {
                        chat.ask_create_channel(&key, &space, Some(channel), ChannelKind::Text);
                    }),
            );
        } else {
            let key = key.clone();
            items.push(
                menu_item(tr!("discord.menu.duplicate-channel"))
                    .icon(MaterialSymbol::CONTENT_COPY)
                    .on_press(move |chat: &mut Chat| chat.clone_channel(&key, &channel)),
            );
        }
        let key = key.clone();
        items.push(
            menu_item(match category {
                true => tr!("discord.menu.delete-category"),
                false => tr!("discord.menu.delete-channel"),
            })
            .icon(MaterialSymbol::DELETE)
            .on_press(move |chat: &mut Chat| chat.ask_delete_channel(&key, &channel)),
        );
    }
    items.push(menu_separator());
    items.push(copy_item(
        tr!("discord.menu.copy-channel-id"),
        channel.to_string(),
    ));
    items
}

fn level_items(key: &str, channel: &Channel) -> Vec<MenuItem> {
    let Some(space) = channel.space else {
        return Vec::new();
    };
    let id = channel.id;
    let inherit = match channel.parent {
        Some(_) => tr!("discord.menu.level-category"),
        None => tr!("discord.menu.level-space"),
    };
    let current = channel.notify.level;
    let choose = |label: Label, level: Option<NotifyLevel>| {
        let key = key.to_owned();
        menu_item(label)
            .selected(current == level)
            .on_press(move |chat: &mut Chat| chat.set_channel_level(&key, &space, &id, level))
    };
    std::iter::once(choose(inherit, None))
        .chain([menu_separator()])
        .chain(
            NotifyLevel::ALL
                .into_iter()
                .map(|level| choose(level_label(level), Some(level))),
        )
        .collect()
}
