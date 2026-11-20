use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_bluetooth::{Prompt, Question, Snapshot};

use crate::bluetooth::component::Bluetooth;

const CODE: f32 = 160.0;

pub(crate) fn view(prompt: &Prompt, typed: &TextDocument, snapshot: &Snapshot) -> impl View {
    let name = snapshot
        .device(&prompt.device)
        .map(|device| device.title.clone())
        .unwrap_or_default();

    card(
        column! {
            text(tr!("settings.bluetooth.pairing.title", device = &name))
                .body()
                .semibold();
            text(question(&prompt.question)).label();
            match &prompt.question {
                Question::Confirm(passkey) => passkey_text(&format(*passkey)).build(),
                Question::Display { passkey, entered } => column! {
                    passkey_text(&format(*passkey));
                    if *entered > 0 {
                        text(tr!(
                            "settings.bluetooth.pairing.entered",
                            entered = &entered.to_string()
                        ))
                        .caption()
                        .muted();
                    }
                }
                .gap(Space::SNUG)
                .fill_width()
                .build(),
                Question::DisplayPin(pin) => passkey_text(pin).build(),
                Question::AskPasskey | Question::AskPin => text_field(typed)
                    .placeholder(match prompt.question == Question::AskPin {
                        true => tr!("settings.bluetooth.pairing.pin-hint"),
                        false => tr!("settings.bluetooth.pairing.passkey-hint"),
                    })
                    .autofocus(true)
                    .on_change(|| {})
                    .on_submit(|bluetooth: &mut Bluetooth| bluetooth.answer(true))
                    .width(CODE * 2.0)
                    .build(),
                Question::Authorize | Question::AuthorizeService(_) => Element::empty(),
            }
            row! {
                spacer();
                if prompt.question.is_answerable() {
                    button(text(tr!("settings.bluetooth.pairing.reject")))
                        .secondary()
                        .on_press(|bluetooth: &mut Bluetooth| bluetooth.answer(false));
                    button(text(match prompt.question.is_asking() {
                        true => tr!("settings.bluetooth.pairing.send"),
                        false => tr!("settings.bluetooth.pairing.accept"),
                    }))
                    .primary()
                    .on_press(|bluetooth: &mut Bluetooth| bluetooth.answer(true));
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn question(question: &Question) -> Label {
    match question {
        Question::Confirm(_) => tr!("settings.bluetooth.pairing.confirm"),
        Question::Display { .. } => tr!("settings.bluetooth.pairing.display"),
        Question::DisplayPin(_) => tr!("settings.bluetooth.pairing.display-pin"),
        Question::AskPasskey => tr!("settings.bluetooth.pairing.ask-passkey"),
        Question::AskPin => tr!("settings.bluetooth.pairing.ask-pin"),
        Question::Authorize => tr!("settings.bluetooth.pairing.authorize"),
        Question::AuthorizeService(_) => tr!("settings.bluetooth.pairing.authorize-service"),
    }
}

fn format(passkey: u32) -> String {
    std::format!("{passkey:06}")
}

fn passkey_text(code: &str) -> impl View {
    container(
        text(code.to_owned())
            .display()
            .bold()
            .align(TextAlign::Center)
            .fill_width(),
    )
    .background(palette().surface_variant)
    .radius(Corner::Panel)
    .padding(Space::NORMAL)
    .fill_width()
}
