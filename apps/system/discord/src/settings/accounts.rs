use discord_core::SavedAccount;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::brand;
use crate::chat::Chat;

pub(super) fn view(chat: &Chat) -> Element {
    column! {
        column_of(chat.accounts().all().iter().map(|saved| account(chat, saved)))
            .gap(Space::SNUG)
            .fill_width();
        button(
            row! {
                icon(brand::mark()).extent(Length::px(18.0)).color(brand::blurple());
                text(tr!("discord.accounts.add")).label();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .secondary()
        .on_press(|chat: &mut Chat| {
            chat.close_settings();
            chat.sign_in_again();
        });
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn account(chat: &Chat, saved: &SavedAccount) -> Element {
    let below: Label = match (
        chat.is_linked(&saved.key),
        chat.connection_error(&saved.key),
    ) {
        (true, _) => tr!("discord.settings.connected"),
        (false, Some(_)) => tr!("discord.accounts.connection-failed"),
        (false, None) => tr!("discord.signing.connecting"),
    };
    let current = chat.current_key() == Some(saved.key.as_str());
    let (chosen, signed_out) = (saved.key.clone(), saved.key.clone());
    card(
        row! {
            column! {
                text(saved.account.name.clone()).label().semibold().no_wrap();
                text(below).caption().muted().no_wrap();
            }
            .grow(1.0)
            .min_width(0.0);
            if !current {
                button(text(tr!("discord.settings.switch")))
                    .ghost()
                    .small()
                    .on_press(move |chat: &mut Chat| {
                        chat.close_settings();
                        chat.switch(&chosen);
                    });
            }
            button(text(tr!("discord.accounts.sign-out")))
                .danger()
                .small()
                .on_press(move |chat: &mut Chat| chat.sign_out(&signed_out));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .any()
}
