use rikka_app::prelude::*;

use super::Shot;
use super::copying::copy_picture;
use crate::chat::Chat;
use crate::conversation::media::open_link;

pub(crate) fn picture_items(shot: &Shot) -> Vec<MenuItem> {
    let copied = shot.full.clone();
    let saved = (shot.url.clone(), shot.name.clone());
    let url_to_copy = shot.url.clone();
    let url_to_open = shot.url.clone();
    vec![
        menu_item(tr!("discord.conversation.image-copy"))
            .icon(MaterialSymbol::CONTENT_COPY)
            .on_press(move || {
                copy_picture(copied.clone());
                skip_view();
            }),
        menu_item(tr!("discord.conversation.image-save"))
            .icon(MaterialSymbol::DOWNLOAD)
            .on_press(move |chat: &mut Chat| chat.save_as(&saved.0, &saved.1)),
        menu_separator(),
        menu_item(tr!("discord.conversation.image-copy-link"))
            .icon(MaterialSymbol::LINK)
            .on_press(move || {
                rikka_ui::clipboard::set_text(&url_to_copy);
                skip_view();
            }),
        menu_item(tr!("discord.conversation.image-open-link"))
            .icon(MaterialSymbol::OPEN_IN_NEW)
            .on_press(move || {
                open_link(&url_to_open);
                skip_view();
            }),
    ]
}
