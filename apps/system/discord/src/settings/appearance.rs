use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::chat::Chat;
use crate::preferences::Appearance;

pub(super) fn view(chat: &Chat) -> Element {
    let appearance = chat.appearance();
    column! {
        section(
            tr!("discord.settings.message-display"),
            segmented(usize::from(appearance.compact))
                .segment(tr!("discord.settings.cozy"))
                .segment(tr!("discord.settings.compact"))
                .on_select(|chat: &mut Chat, at: usize| {
                    let mut appearance = chat.appearance();
                    appearance.compact = at == 1;
                    chat.set_appearance(appearance);
                })
                .width(260.0),
        );
        section(
            tr!("discord.settings.content"),
            column! {
                toggle(tr!("discord.settings.show-embeds"), appearance.embeds, |appearance, on| {
                    appearance.embeds = on;
                });
                toggle(tr!("discord.settings.animated-emoji"), appearance.animated_emoji, |appearance, on| {
                    appearance.animated_emoji = on;
                });
            }
            .gap(Space::SNUG)
            .fill_width(),
        );
    }
    .gap(Space::LOOSE)
    .fill_width()
    .any()
}

fn toggle(label: Label, on: bool, set: fn(&mut Appearance, bool)) -> impl View {
    field(
        label,
        switch(on).on_toggle(move |chat: &mut Chat, on: bool| {
            let mut appearance = chat.appearance();
            set(&mut appearance, on);
            chat.set_appearance(appearance);
        }),
    )
}
