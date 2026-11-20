use discord_client::writing;
use discord_core::{Abilities, ChannelKind, Delivery, MessageId};
use rikka_app::prelude::*;
use rikka_ui::Element;

use super::{Overlay, OverlayAnchor, PickerTarget, Tab};
use crate::chat::Chat;
use crate::conversation::composer::sendable;
use crate::conversation::lightbox::{Shot, picture_items};

const WIDTH: f32 = 232.0;
const EXCERPT: usize = 40;

pub(super) fn view(
    chat: &Chat,
    message: &MessageId,
    anchor: &OverlayAnchor,
    shift: bool,
    picture: Option<&Shot>,
) -> Option<Element> {
    let (key, channel) = chat.conversation().shown()?.clone();
    let linked = chat.linked(&key)?;
    let channel_info = linked.channel(&channel)?;
    let target = linked.timeline(&channel)?.find(message)?.clone();
    let abilities = channel_info.abilities;
    let mine = linked.is_me(&target.author.id);
    let delivered = target.delivery == Delivery::Delivered;
    let id = *message;
    let mut items = Vec::new();
    if delivered && abilities.contains(Abilities::REACT) {
        items.push(
            menu_item(tr!("discord.conversation.add-reaction"))
                .icon(MaterialSymbol::ADD_REACTION)
                .on_press(move |overlay: &mut Overlay| {
                    overlay.open_picker(PickerTarget::React(id), Tab::Emoji)
                }),
        );
    }
    if delivered && sendable(channel_info) {
        items.push(
            menu_item(tr!("discord.conversation.reply"))
                .icon(MaterialSymbol::REPLY)
                .on_press(move |chat: &mut Chat| chat.reply_to(&id)),
        );
    }
    if delivered && mine && !target.is_system() {
        items.push(
            menu_item(tr!("discord.conversation.edit"))
                .icon(MaterialSymbol::EDIT)
                .on_press(move |chat: &mut Chat| chat.start_edit(&id)),
        );
    }
    let threads_here = channel_info.space.is_some()
        && !matches!(channel_info.kind, ChannelKind::Thread | ChannelKind::Forum);
    if delivered
        && threads_here
        && target.thread.is_none()
        && abilities.contains(Abilities::CREATE_THREADS)
    {
        let name: String = target.content.plain_text().chars().take(EXCERPT).collect();
        items.push(
            menu_item(tr!("discord.conversation.create-thread"))
                .icon(MaterialSymbol::FORUM)
                .on_press(move |overlay: &mut Overlay| {
                    overlay.ask_new_thread(Some(id), name.trim())
                }),
        );
    }
    if delivered && abilities.contains(Abilities::PIN) {
        let pinned = target.pinned;
        items.push(
            menu_item(match pinned {
                true => tr!("discord.conversation.unpin"),
                false => tr!("discord.conversation.pin"),
            })
            .icon(MaterialSymbol::PUSH_PIN)
            .on_press(move |chat: &mut Chat| chat.pin(&id, !pinned)),
        );
    }
    items.push(menu_separator());
    let caption = target.text.clone();
    items.push(
        menu_item(tr!("discord.conversation.copy-text"))
            .icon(MaterialSymbol::CONTENT_COPY)
            .on_press(move || {
                rikka_ui::clipboard::set_text(&caption);
                skip_view();
            }),
    );
    if delivered {
        let link = writing::message_link(channel_info, message);
        items.push(
            menu_item(tr!("discord.conversation.copy-link"))
                .icon(MaterialSymbol::LINK)
                .on_press(move || {
                    rikka_ui::clipboard::set_text(&link);
                    skip_view();
                }),
        );
    }
    if delivered {
        let copied = id.to_string();
        items.push(
            menu_item(tr!("discord.conversation.copy-id"))
                .icon(MaterialSymbol::TAG)
                .on_press(move || {
                    rikka_ui::clipboard::set_text(&copied);
                    skip_view();
                }),
        );
        let unread = id;
        items.push(
            menu_item(tr!("discord.conversation.mark-unread"))
                .icon(MaterialSymbol::MARK_CHAT_UNREAD)
                .on_press(move |chat: &mut Chat| chat.mark_unread(&unread)),
        );
    }
    if let Some(picture) = picture {
        items.push(menu_separator());
        items.extend(picture_items(picture));
    }
    if mine || abilities.contains(Abilities::MANAGE_MESSAGES) {
        items.push(menu_separator());
        items.push(
            menu_item(tr!("discord.conversation.delete"))
                .icon(MaterialSymbol::DELETE)
                .on_press(move |chat: &mut Chat| chat.ask_delete(&id, shift)),
        );
    }
    Some(
        menu()
            .items(items)
            .width(WIDTH)
            .on_activate(|overlay: &mut Overlay, _: usize| overlay.close_menu())
            .on_dismiss(|overlay: &mut Overlay| overlay.close_menu())
            .build()
            .keyed(("discord-menu", id))
            .anchored(anchor.anchor()),
    )
}
