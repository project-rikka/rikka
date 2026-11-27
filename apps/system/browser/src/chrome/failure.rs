use rikka_app::prelude::*;
use rikka_web::Failure;
use rikka_widgets::column;

use crate::shell::Shell;

const WIDEST: f32 = 520.0;

pub(crate) fn failure_page(failure: &Failure) -> Element {
    let (title, detail) = match failure {
        Failure::ExtensionMissing => (
            tr!("browser.chrome.missing"),
            tr!("browser.chrome.missing-detail"),
        ),
        Failure::ExtensionOutdated { found, expected } => (
            tr!("browser.chrome.outdated"),
            tr!(
                "browser.chrome.outdated-detail",
                found = &found.to_string(),
                expected = &expected.to_string()
            ),
        ),
    };
    status_page(MaterialSymbol::EXTENSION_OFF, title)
        .tint(palette().danger)
        .action(
            column! {
                text(detail).muted().fill_width();
                text(tr!("browser.chrome.repair")).caption().muted().fill_width();
            }
            .gap(Space::SNUG)
            .max_width(WIDEST),
        )
        .any()
}

pub(crate) fn exited_page() -> Element {
    status_page(MaterialSymbol::WARNING, tr!("browser.chrome.exited"))
        .action(
            button(text(tr!("browser.chrome.restart")))
                .primary()
                .on_press(|shell: &mut Shell| shell.changed()),
        )
        .any()
}

pub(crate) fn unavailable_page(error: &str) -> Element {
    status_page(MaterialSymbol::WARNING, tr!("browser.chrome.unavailable"))
        .tint(palette().danger)
        .action(text(error.to_owned()).caption().muted().max_width(WIDEST))
        .any()
}
