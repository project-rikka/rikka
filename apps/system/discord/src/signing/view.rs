use discord_client::Method;
use rikka_app::prelude::*;
use rikka_web::web_view;
use rikka_widgets::{column, row};

use super::{Signing, Step};
use crate::brand;
use crate::chat::Chat;
use crate::widgets::avatar;

const WIDTH: f32 = 440.0;
const CODE: f32 = 224.0;
const SCANNED: f32 = 72.0;

pub(crate) fn view(chat: &Chat) -> Element {
    let signing = chat.signing();
    match (signing.step(), signing.portal_session()) {
        (Step::Page, Some(page)) => portal(chat, page),
        _ => centered(chat),
    }
}

fn centered(chat: &Chat) -> Element {
    let signing = chat.signing();
    let error_message = signing.error_message().or_else(|| chat.error_message());
    let progress = match signing.method() {
        None => welcome(),
        Some(_) => column! {
            tabs_of(signing);
            step(signing);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width()
        .any(),
    };
    let body = column! {
        heading();
        progress;
        if let Some(error_message) = error_message {
            text(error_message.to_owned()).caption().danger().align(TextAlign::Center).fill_width();
        }
        if chat.has_desk() {
            button(text(tr!("discord.signing.cancel")))
                .ghost()
                .on_press(|chat: &mut Chat| chat.leave_sign_in());
        }
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width();
    container(
        container(body)
            .padding(Inset::all(Space::WIDE))
            .background(palette().surface)
            .radius(Corner::Panel)
            .width(WIDTH),
    )
    .center()
    .fill()
    .any()
}

fn heading() -> impl View {
    row! {
        icon(brand::mark()).extent(Length::px(28.0)).color(brand::blurple());
        text(tr!("discord.signing.title")).title().bold().no_wrap();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn welcome() -> Element {
    let choices = column_of(Method::ALL.into_iter().map(|method| {
        button(
            row! {
                icon(method_symbol(method)).extent(Length::px(22.0)).color(palette().accent);
                text(method_label(method)).label().semibold();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .large()
        .secondary()
        .on_press(move |chat: &mut Chat| chat.sign_in_with(method))
        .fill_width()
        .any()
    }))
    .gap(Space::SNUG)
    .fill_width();
    column! {
        text(tr!("discord.signing.choose")).label().muted().align(TextAlign::Center).fill_width();
        choices;
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn tabs_of(signing: &Signing) -> Element {
    let chosen = signing
        .method()
        .and_then(|method| Method::ALL.iter().position(|known| *known == method))
        .unwrap_or(0);
    segmented(chosen)
        .segments(Method::ALL.into_iter().map(method_label))
        .on_select(|chat: &mut Chat, at: usize| {
            if let Some(method) = Method::ALL.get(at) {
                chat.sign_in_with(*method);
            }
        })
        .fill_width()
        .any()
}

fn method_label(method: Method) -> Label {
    match method {
        Method::QrCode => tr!("discord.signing.method.qr"),
        Method::Browser => tr!("discord.signing.method.browser"),
    }
}

fn method_symbol(method: Method) -> MaterialSymbol {
    match method {
        Method::QrCode => MaterialSymbol::QR_CODE,
        Method::Browser => MaterialSymbol::LANGUAGE,
    }
}

fn step(signing: &Signing) -> Element {
    match signing.step() {
        Step::Starting => waiting(match signing.method() {
            Some(Method::Browser) => tr!("discord.signing.opening-page"),
            Some(Method::QrCode) | None => tr!("discord.signing.preparing-code"),
        }),
        Step::Code(picture) => column! {
            image(picture.clone())
                .fit(ImageFit::Contain)
                .radius(Corner::Control.radius())
                .size(Length::px(CODE), Length::px(CODE));
            text(tr!("discord.signing.scan"))
                .label()
                .muted()
                .align(TextAlign::Center)
                .fill_width();
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width()
        .any(),
        Step::Scanned { name, avatar: face } => column! {
            avatar(face.as_ref(), name, SCANNED);
            text(name.clone()).heading().semibold();
            text(tr!("discord.signing.check-phone")).label().muted();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
        .any(),
        Step::Page => waiting(tr!("discord.signing.opening-page")),
        Step::Stopped => {
            let method = signing.method();
            button(text(tr!("discord.signing.again")))
                .secondary()
                .on_press(move |chat: &mut Chat| {
                    if let Some(method) = method {
                        chat.sign_in_with(method);
                    }
                })
                .any()
        }
    }
}

fn waiting(caption: Label) -> Element {
    column! {
        spinner();
        text(caption).label().muted();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NONE, Space::LOOSE))
    .fill_width()
    .any()
}

fn portal(chat: &Chat, page: &rikka_web::Session) -> Element {
    let signing = chat.signing();
    column! {
        row! {
            icon(brand::mark()).extent(Length::px(20.0)).color(brand::blurple());
            text(tr!("discord.signing.title")).label().semibold().no_wrap();
            spacer().grow(1.0);
            container(tabs_of(signing)).width(320.0);
            if chat.has_desk() {
                button(text(tr!("discord.signing.cancel")))
                    .ghost()
                    .on_press(|chat: &mut Chat| chat.leave_sign_in());
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::LOOSE, Space::SNUG))
        .fill_width();
        web_view(page).grow(1.0).fill_width();
    }
    .fill()
    .any()
}
