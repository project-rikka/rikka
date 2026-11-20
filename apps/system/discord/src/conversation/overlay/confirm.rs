use discord_core::MessageId;
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use super::Overlay;
use crate::chat::Chat;
use crate::conversation::{AREA, time};
use crate::widgets::avatar;

const WIDTH: f32 = 440.0;
const EXCERPT: usize = 300;

pub(super) fn view(chat: &Chat, message: &MessageId) -> Option<Element> {
    let (key, channel) = chat.conversation().shown()?.clone();
    let linked = chat.linked(&key)?;
    let space = linked
        .channel(&channel)
        .and_then(|channel_info| channel_info.space);
    let target = linked.timeline(&channel)?.find(message)?.clone();
    let name = linked.display_name(space.as_ref(), &target.author);
    let excerpt: String = target.content.plain_text().chars().take(EXCERPT).collect();
    let id = *message;
    Some(
        popover(
            column! {
                text(tr!("discord.conversation.delete-title")).heading().semibold();
                text(tr!("discord.conversation.delete-ask")).body().muted().fill_width();
                container(
                    row! {
                        avatar(target.author.avatar.as_ref(), &name, 32.0);
                        column! {
                            row! {
                                text(name.clone()).label().semibold().no_wrap();
                                text(time::sent_at(target.at)).caption().muted().no_wrap();
                            }
                            .gap(Space::SNUG);
                            text(excerpt).body().fill_width();
                        }
                        .gap(Space::HAIR)
                        .grow(1.0)
                        .min_width(0.0);
                    }
                    .gap(Space::SNUG)
                    .fill_width(),
                )
                .padding(Inset::all(Space::SNUG))
                .background(palette().surface_variant.with_alpha(0.5))
                .radius(Corner::Item)
                .fill_width();
                text(tr!("discord.conversation.delete-tip")).caption().muted().fill_width();
                row! {
                    spacer().grow(1.0);
                    button(text(tr!("discord.conversation.cancel")))
                        .ghost()
                        .on_press(|overlay: &mut Overlay| overlay.close());
                    button(text(tr!("discord.conversation.delete")))
                        .danger()
                        .on_press(move |chat: &mut Chat| chat.delete_message(&id));
                }
                .gap(Space::SNUG)
                .fill_width();
            }
            .gap(Space::NORMAL)
            .width(WIDTH),
        )
        .on_dismiss(|overlay: &mut Overlay| overlay.close())
        .build()
        .anchored(Anchor::centered_on(AREA)),
    )
}
