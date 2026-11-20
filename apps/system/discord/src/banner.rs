use discord_core::LinkState;
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::chat::Chat;

pub(crate) fn view(chat: &Chat) -> Option<Element> {
    let linked = chat.current()?;
    let (symbol, tint, caption, again) = match linked.link() {
        LinkState::Online => return error_message(chat),
        LinkState::Connecting => (
            MaterialSymbol::SYNC,
            palette().accent,
            tr!("discord.link.connecting"),
            false,
        ),
        LinkState::Reconnecting { attempt } => (
            MaterialSymbol::WIFI_OFF,
            palette().warning,
            tr!("discord.link.reconnecting", attempt = attempt as i64),
            false,
        ),
        LinkState::SignedOut => (
            MaterialSymbol::LOGIN,
            palette().danger,
            tr!(
                "discord.link.signed-out",
                name = linked.account().name.as_str()
            ),
            true,
        ),
    };
    let forgotten = linked.key().to_owned();
    Some(
        container(
            row! {
                icon(symbol).extent(Length::px(18.0)).color(tint);
                text(caption).label().medium().grow(1.0).min_width(0.0);
                if again {
                    button(text(tr!("discord.link.sign-in-again")))
                        .small()
                        .primary()
                        .on_press(|chat: &mut Chat| chat.sign_in_again());
                    button(text(tr!("discord.link.forget")))
                        .small()
                        .ghost()
                        .on_press(move |chat: &mut Chat| chat.forget(&forgotten));
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        )
        .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
        .background(tint.with_alpha(0.16))
        .fill_width()
        .transition(fade())
        .any(),
    )
}

fn error_message(chat: &Chat) -> Option<Element> {
    let caption = chat.error_message()?.to_owned();
    Some(
        banner(caption)
            .on_dismiss(tr!("discord.link.dismiss"), |chat: &mut Chat| {
                chat.dismiss_trouble()
            })
            .fill_width()
            .any(),
    )
}
