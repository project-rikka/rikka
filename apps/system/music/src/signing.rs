use music_core::Service;
use rikka_app::prelude::*;
use rikka_web::web_view;
use rikka_widgets::{column, row};

use crate::accounts;
use crate::marks::mark;
use crate::music::{Music, Stage};

const WIDTH: f32 = 420.0;

pub(crate) fn view(music: &Music) -> Element {
    match music.stage() {
        Stage::Page(service) => page(music, *service),
        _ => centered(music),
    }
}

fn centered(music: &Music) -> Element {
    container(
        column! {
            icon(MaterialSymbol::GRAPHIC_EQ).extent(Length::px(56.0)).color(palette().accent);
            text(tr!("music.title")).display().bold();
            spacer().height(Space::SNUG.px());
            step(music);
            if let Some(error_message) = music.error_message() {
                text(error_message.to_owned())
                    .caption()
                    .danger()
                    .align(TextAlign::Center)
                    .fill_width();
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .width(WIDTH),
    )
    .center()
    .fill()
    .any()
}

fn step(music: &Music) -> Element {
    match music.stage() {
        Stage::Connecting | Stage::Ready | Stage::Page(_) => column! {
            spinner();
            text(tr!("music.signing.connecting")).label().muted();
            button(text(tr!("music.signing.cancel")))
                .ghost()
                .on_press(|music: &mut Music| music.cancel_connecting());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .any(),
        Stage::Choosing => choosing(music),
        Stage::Browser { service, url } => {
            let again = url.clone();
            column! {
                spinner();
                text(tr!("music.signing.browser", name = service.name))
                    .label()
                    .muted()
                    .align(TextAlign::Center)
                    .fill_width();
                row! {
                    button(text(tr!("music.signing.again")))
                        .secondary()
                        .on_press(move || {
                            if let Err(error) = rikka_open::open(&again) {
                                tracing::warn!("failed to open sign-in page: {error}");
                            }
                            skip_view();
                        });
                    button(text(tr!("music.signing.cancel")))
                        .ghost()
                        .on_press(|music: &mut Music| music.cancel_connecting());
                }
                .gap(Space::SNUG);
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .fill_width()
            .any()
        }
    }
}

fn choosing(music: &Music) -> Element {
    column! {
        if !music.accounts().all().is_empty() {
            heading(tr!("music.accounts.saved"));
            accounts::saved(music);
            heading(tr!("music.accounts.add"));
        }
        accounts::services(music.providers());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn heading(title: Label) -> impl View {
    container(text(title).caption().muted().semibold())
        .padding(Inset::edges(
            Space::SNUG,
            Space::NONE,
            Space::HAIR,
            Space::TIGHT,
        ))
        .fill_width()
}

fn page(music: &Music, service: Service) -> Element {
    let Some(portal) = music.portal() else {
        return centered(music);
    };
    column! {
        row! {
            icon(mark(service.mark)).extent(Length::px(20.0)).color(palette().accent);
            text(tr!("music.signing.page", name = service.name)).label().semibold().grow(1.0);
            button(text(tr!("music.signing.cancel")))
                .ghost()
                .on_press(|music: &mut Music| music.cancel_connecting());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::LOOSE, Space::SNUG))
        .fill_width();
        web_view(portal.session()).grow(1.0).fill_width();
    }
    .fill()
    .any()
}
