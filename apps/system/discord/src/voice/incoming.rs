use discord_core::{ChannelId, ChannelKind, Picture};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::chat::Chat;
use crate::widgets::avatar;

const WIDTH: f32 = 300.0;
const FACE: f32 = 72.0;

#[derive(Clone, Debug)]
struct Ringing {
    key: String,
    channel: ChannelId,
    title: String,
    account: Option<String>,
    picture: Option<Picture>,
}

pub(crate) fn view(chat: &Chat) -> Option<Element> {
    let ringing = rung(chat);
    if ringing.is_empty() {
        return None;
    }
    Some(
        column_of(ringing.into_iter().map(card))
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .any(),
    )
}

fn rung(chat: &Chat) -> Vec<Ringing> {
    let current = chat.current_key();
    chat.links()
        .iter()
        .flat_map(|linked| {
            linked
                .incoming_calls()
                .into_iter()
                .filter(|ringing| !chat.calls().is_seated_in(linked.key(), &ringing.channel))
                .filter_map(|ringing| {
                    let channel = linked.channel(&ringing.channel)?;
                    let picture = match channel.kind {
                        ChannelKind::Direct => channel
                            .recipients
                            .first()
                            .and_then(|caller| linked.user(caller))
                            .and_then(|caller| caller.avatar.clone()),
                        _ => channel.icon.clone(),
                    };
                    Some(Ringing {
                        key: linked.key().to_owned(),
                        channel: ringing.channel,
                        title: linked.channel_title(channel),
                        account: (current != Some(linked.key()))
                            .then(|| linked.account().name.clone()),
                        picture,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn card(ringing: Ringing) -> Element {
    let (accepted, declined) = (ringing.clone(), ringing.clone());
    container(
        column! {
            avatar(ringing.picture.as_ref(), &ringing.title, FACE);
            text(ringing.title.clone()).heading().semibold().no_wrap();
            text(tr!("discord.voice.incoming")).caption().muted();
            if let Some(account) = &ringing.account {
                text(tr!("discord.voice.on-account", name = account.as_str())).caption().muted();
            }
            row! {
                button(icon(MaterialSymbol::CALL_END).extent(Length::px(22.0)).color(palette().on_danger))
                    .danger()
                    .icon_only()
                    .on_press(move |chat: &mut Chat| chat.decline_call(&declined.key, &declined.channel));
                button(icon(MaterialSymbol::CALL).extent(Length::px(22.0)).color(palette().on_success))
                    .primary()
                    .icon_only()
                    .on_press(move |chat: &mut Chat| chat.accept_call(&accepted.key, &accepted.channel));
            }
            .gap(Space::LOOSE);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::all(Space::ROOMY))
    .background(palette().surface)
    .border(1.0, palette().outline)
    .radius(Corner::Panel)
    .width(WIDTH)
    .any()
}
