use music_core::{Providers, SavedAccount};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::marks::{mark, mark_of};
use crate::music::Music;

const ACCOUNT: f32 = 56.0;

struct Line {
    key: String,
    icon: IconSource,
    name: Label,
    detail: Label,
    forgettable: bool,
}

pub(crate) fn saved(music: &Music) -> impl View {
    let current = music.account_key().to_owned();
    let local = music
        .local_key()
        .filter(|key| music.is_linked(key))
        .map(|key| local_line(music, key));
    column_of(
        local
            .into_iter()
            .chain(
                music
                    .accounts()
                    .all()
                    .iter()
                    .map(|saved| saved_account_line(saved, music.providers())),
            )
            .map(|line| {
                let current = line.key == current;
                account(line, current)
            }),
    )
    .gap(Space::HAIR)
    .fill_width()
}

pub(crate) fn services(providers: &Providers) -> impl View {
    column_of(
        providers
            .services()
            .filter(|service| !service.is_local())
            .map(|service| {
                button(
                    row! {
                        icon(mark(service.mark)).extent(Length::px(20.0));
                        text(tr!("music.signing.with", name = service.name)).label().semibold();
                    }
                    .gap(Space::SNUG)
                    .align_items(Align::Center),
                )
                .large()
                .on_press(move |music: &mut Music| music.sign_in(service.slug))
                .fill_width()
                .build()
            }),
    )
    .gap(Space::SNUG)
    .fill_width()
}

fn local_line(music: &Music, key: String) -> Line {
    let progress = music.scanning();
    Line {
        icon: mark_of(music.providers(), &key),
        key,
        name: tr!("music.local.name"),
        detail: match progress.scanning {
            true => tr!(
                "music.local.scanning",
                read = &progress.read.to_string(),
                total = &progress.to_read.to_string()
            ),
            false => tr!("music.local.here"),
        },
        forgettable: false,
    }
}

fn saved_account_line(saved: &SavedAccount, providers: &Providers) -> Line {
    let service = providers.find(&saved.provider).map_or_else(
        || saved.provider.clone(),
        |registered| registered.service().name.to_owned(),
    );
    Line {
        key: saved.key.clone(),
        icon: mark_of(providers, &saved.provider),
        name: Label::from(saved.account.name.clone()),
        detail: Label::from(service),
        forgettable: true,
    }
}

fn account(line: Line, current: bool) -> Element {
    let chosen = line.key.clone();
    let forgotten = line.key;
    container(
        row! {
            icon(line.icon).extent(Length::px(22.0)).color(palette().accent);
            column! {
                text(line.name).label().medium().no_wrap();
                text(line.detail).caption().muted().no_wrap();
            }
            .grow(1.0)
            .min_width(0.0);
            if current {
                icon(MaterialSymbol::CHECK_CIRCLE)
                    .extent(Length::px(20.0))
                    .color(palette().accent)
                    .filled();
            }
            if line.forgettable {
                button(icon(MaterialSymbol::LOGOUT).extent(Length::px(18.0)))
                    .ghost()
                    .icon_only()
                    .small()
                    .on_press(move |music: &mut Music| music.forget(&forgotten));
            }
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
    .radius(Corner::Panel)
    .background(match current {
        true => palette().accent.with_alpha(0.12),
        false => palette().surface_variant.with_alpha(0.5),
    })
    .on_press(move |music: &mut Music| music.switch(&chosen))
    .height(ACCOUNT)
    .fill_width()
    .any()
}
