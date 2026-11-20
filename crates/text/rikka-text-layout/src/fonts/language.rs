use std::str::FromStr;
use std::sync::OnceLock;

use rikka_text_catalog::catalog;
use rikka_text_shape::Language;

pub(crate) fn shaping_language() -> Option<Language> {
    static LANGUAGE: OnceLock<Option<Language>> = OnceLock::new();
    *LANGUAGE.get_or_init(|| {
        let locale = catalog().locale()?;
        Language::from_str(&locale.to_string()).ok()
    })
}
