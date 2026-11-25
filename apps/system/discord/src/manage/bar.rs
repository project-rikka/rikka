use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::chat::Chat;

pub(crate) fn unsaved_bar(
    warned: bool,
    saving: bool,
    reset: impl Fn(&mut Chat) + 'static,
    save: impl Fn(&mut Chat) + 'static,
) -> Element {
    let tint = match warned {
        true => palette().danger,
        false => palette().surface_variant,
    };
    container(
        row! {
            text(match warned {
                true => tr!("discord.admin.unsaved-warning"),
                false => tr!("discord.admin.unsaved"),
            })
            .label()
            .semibold()
            .grow(1.0)
            .min_width(0.0);
            button(text(tr!("discord.admin.reset")))
                .ghost()
                .small()
                .enabled(!saving)
                .on_press(reset);
            button(text(tr!("discord.admin.save")))
                .primary()
                .small()
                .enabled(!saving)
                .on_press(save);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .background(tint.with_alpha(0.9))
    .radius(Corner::Control)
    .fill_width()
    .transition(fade())
    .any()
}
