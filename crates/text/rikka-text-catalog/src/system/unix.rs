use rikka_text_unicode::Script;

use crate::cjk::CjkRegion;
use crate::config::{Environment, Fontconfig};
use crate::family::Family;
use crate::locale::Locale;
use crate::report::LoadProblem;
use crate::system::SystemFonts;

pub(crate) fn fonts(environment: &Environment, problems: &mut Vec<LoadProblem>) -> SystemFonts {
    SystemFonts {
        fontconfig: Fontconfig::load(&environment.main_config(), environment, problems),
        ..SystemFonts::default()
    }
}

pub(crate) fn locale() -> Option<Locale> {
    Locale::parse(&rikka_types::LocaleCategory::Ctype.posix_name()?)
}

pub(crate) fn preferred_families(generic: &Family) -> &'static [&'static str] {
    match generic {
        Family::SansSerif | Family::SystemUi => &["Inter"],
        Family::Monospace => &["JetBrains Mono"],
        Family::Serif => &["Noto Serif"],
        Family::Emoji => &["Noto Color Emoji"],
        _ => &[],
    }
}

pub(crate) fn fallback_families(_: Script, _: Option<CjkRegion>) -> &'static [&'static str] {
    &[]
}
