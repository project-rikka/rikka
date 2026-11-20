use rikka_llm::{Trouble, Window};
use rikka_llm_accounts::{Reading, verification_label};
use rikka_session::settings_app::{Link, Section};
use rikka_ui::{
    Align, Element, Inset, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text,
    tr,
};
use rikka_usage_bar::usage_bar;
use rikka_widgets::{ControlSize, button, column, icon, row};

use crate::indicators::Shelf;
use crate::indicators::parts::{entry, rule};
use crate::indicators::usage::{Usage, UsageSource};

const INSET: f32 = 10.0;
const REFRESH: f32 = 16.0;

pub(crate) fn view(usage: &Usage, shelf: &Shelf) -> impl View {
    column! {
        header(usage);
        for metered in usage.metered() {
            account(usage, &metered, shelf);
        }
        rule();
        entry(tr!("shell.indicators.ai-usage.settings"), {
            let shelf = shelf.clone();
            move || shelf.launch(Link::Section(Section::Llm).argv())
        })
        .trailing(MaterialSymbol::OPEN_IN_NEW);
    }
    .gap(Space::HAIR)
    .align_items(Align::Stretch)
    .fill_width()
}

fn header(usage: &Usage) -> impl View {
    let refreshing = usage.clone();
    row! {
        text(tr!("shell.indicators.ai-usage.title")).label().medium().no_wrap().grow(1.0);
        button(icon(MaterialSymbol::REFRESH).extent(Length::px(REFRESH)))
            .icon_only()
            .ghost()
            .control_size(ControlSize::Small)
            .on_press(move || refreshing.refresh());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .padding(Inset::edges(Space::TIGHT, Space::TIGHT, 0.0, INSET))
}

fn account(usage: &Usage, metered: &UsageSource, shelf: &Shelf) -> Element {
    usage.with_reading(metered.account, |reading| {
        let windows = reading.map_or(&[] as &[Window], Reading::windows);
        column! {
            row! {
                text(metered.label.as_str()).label().medium().no_wrap();
                text(metered.who.as_str()).caption().muted().no_wrap().grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            reading.and_then(Reading::error).map(|reported| trouble(reported, shelf));
            for window in windows {
                usage_bar(window).compact();
            }
            if windows.is_empty() && reading.is_none_or(|reading| reading.error().is_none()) {
                text(match reading.is_some_and(Reading::is_pending) {
                    true => tr!("shell.indicators.ai-usage.asking"),
                    false => tr!("shell.indicators.ai-usage.unmetered"),
                })
                .caption()
                .muted();
            }
        }
        .gap(Space::SNUG)
        .fill_width()
        .padding(Inset::xy(INSET, Space::SNUG))
        .build()
    })
}

fn trouble(trouble: &Trouble, shelf: &Shelf) -> impl View {
    let verify = trouble.verification_url().map(|url| {
        let shelf = shelf.clone();
        let opener = rikka_open::opener(url);
        button(text(verification_label()).caption())
            .secondary()
            .control_size(ControlSize::Small)
            .on_press(move || shelf.launch(opener.clone()))
    });
    row! {
        text(trouble.reason().to_owned()).caption().color(palette().warning).grow(1.0);
        verify;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}
