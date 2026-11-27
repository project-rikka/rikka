use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::desk::STAGE;
use crate::passwords::{Editing, PasswordState};

const WIDTH: f32 = 480.0;
const NOTE_HEIGHTS: std::ops::RangeInclusive<f32> = 72.0..=180.0;
const KEY: &str = "google-passwords-editing";

pub(crate) fn sheet(editing: &Editing) -> Vec<Element> {
    let title = match editing.replacing {
        Some(_) => tr!("google.passwords.editor.edit", site = editing.site.as_str()),
        None => tr!("google.passwords.editor.add"),
    };
    let reveal = match editing.revealed {
        true => (MaterialSymbol::VISIBILITY_OFF, tr!("google.passwords.hide")),
        false => (MaterialSymbol::VISIBILITY, tr!("google.passwords.show")),
    };
    let body = column! {
        if editing.replacing.is_none() {
            labeled(
                tr!("google.passwords.editor.site"),
                text_field(&editing.address)
                    .placeholder(tr!("google.passwords.editor.site-hint"))
                    .autofocus(true)
                    .fill_width(),
            );
        }
        labeled(
            tr!("google.passwords.username"),
            text_field(&editing.username)
                .autofocus(editing.replacing.is_some())
                .fill_width(),
        );
        labeled(
            tr!("google.passwords.password"),
            row! {
                text_field(&editing.password)
                    .secret(!editing.revealed)
                    .grow(1.0);
                button(icon(reveal.0).extent(Length::px(16.0)))
                    .ghost()
                    .icon_only()
                    .on_press(PasswordState::editing_revealed)
                    .tooltip(reveal.1);
                button(icon(MaterialSymbol::CASINO).extent(Length::px(16.0)))
                    .ghost()
                    .icon_only()
                    .on_press(PasswordState::generate)
                    .tooltip(tr!("google.passwords.editor.generate"));
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width(),
        );
        labeled(
            tr!("google.passwords.note"),
            text_editor(&editing.note)
                .fits_text(NOTE_HEIGHTS)
                .fill_width(),
        );
        if let Some(refusal) = &editing.refusal {
            text(refusal.clone()).caption().danger();
        }
    }
    .gap(Space::NORMAL)
    .fill_width();
    dialog(title, body)
        .cancel(tr!("google.cancel"), PasswordState::cancel_editing)
        .accept(tr!("google.passwords.editor.save"), PasswordState::save)
        .width(WIDTH)
        .keyed((KEY, editing.key))
        .centered_on(STAGE)
        .with_scrim()
}

fn labeled(label: Label, control: impl View) -> impl View {
    column! {
        text(label).caption().muted();
        control;
    }
    .gap(Space::TIGHT)
    .fill_width()
}
