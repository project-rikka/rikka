use browser_translators::{CodeMode, Display};
use rikka_app::prelude::*;

pub(crate) fn display_name(display: Display) -> Label {
    match display {
        Display::Replace => tr!("browser.translate.replace"),
        Display::Bilingual => tr!("browser.translate.bilingual"),
    }
}

pub(crate) fn code_name(code: CodeMode) -> Label {
    match code {
        CodeMode::Comments => tr!("browser.translate.comments"),
        CodeMode::Skip => tr!("browser.translate.skip-code"),
    }
}
