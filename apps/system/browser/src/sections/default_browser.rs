use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::default_browser::{Attempt, DefaultStatus, failure_text};
use crate::shell::Shell;

const GLYPH: f32 = 24.0;

pub(crate) fn view(shell: &Shell) -> Element {
    let status = shell.browser().services().default_browser.status();
    let attempt = shell.claim.attempt();
    let asking = shell.browser().preferences().check_default();
    let failure = match &attempt {
        Attempt::Failed(failure) => banner(failure_text(failure)).any(),
        Attempt::Idle | Attempt::Making => Element::empty(),
    };
    page([section(
        tr!("browser.default.title"),
        column! {
            status_line(&status, &attempt);
            failure;
            field(
                tr!("browser.default.ask"),
                switch(asking).on_toggle(|shell: &mut Shell, on: bool| {
                    shell.browser().preferences().set_check_default(on);
                }),
            );
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )])
    .any()
}

fn status_line(status: &DefaultStatus, attempt: &Attempt) -> Element {
    let (symbol, tint, headline, detail) = match status {
        DefaultStatus::Unchecked => (
            MaterialSymbol::OPEN_IN_BROWSER,
            palette().text_muted,
            tr!("browser.default.checking"),
            None,
        ),
        DefaultStatus::Default => (
            MaterialSymbol::CHECK_CIRCLE,
            palette().success,
            tr!("browser.default.is-default"),
            None,
        ),
        DefaultStatus::Other(current) => (
            MaterialSymbol::OPEN_IN_BROWSER,
            palette().text_muted,
            tr!("browser.default.not-default"),
            Some(match current {
                Some(name) => tr!("browser.default.opened-by", name = name.as_str()),
                None => tr!("browser.default.opened-by-none"),
            }),
        ),
        DefaultStatus::Unavailable => (
            MaterialSymbol::BLOCK,
            palette().text_muted,
            tr!("browser.default.not-default"),
            Some(tr!("browser.default.unavailable")),
        ),
    };
    let action = match status {
        DefaultStatus::Other(_) => button(text(tr!("browser.default.make")))
            .primary()
            .small()
            .enabled(*attempt != Attempt::Making)
            .on_press(|shell: &mut Shell| shell.make_default_browser())
            .any(),
        DefaultStatus::Unchecked | DefaultStatus::Default | DefaultStatus::Unavailable => {
            Element::empty()
        }
    };
    let detail = match detail {
        Some(detail) => text(detail).caption().muted().any(),
        None => Element::empty(),
    };
    row! {
        icon(symbol).extent(Length::px(GLYPH)).color(tint);
        column! {
            text(headline).label().medium();
            detail;
        }
        .gap(Space::HAIR)
        .grow(1.0);
        action;
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
    .any()
}
