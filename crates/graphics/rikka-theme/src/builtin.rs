use std::sync::LazyLock;

use crate::spec::ThemeSpec;

include!("builtin_sources.rs");

pub(crate) const LIGHT_SLUG: &str = "rikka-light";
pub(crate) const DARK_SLUG: &str = "rikka";

static BUILTIN: LazyLock<Vec<ThemeSpec>> = LazyLock::new(|| {
    BUILTIN_SOURCES
        .iter()
        .map(|(stem, source)| {
            ThemeSpec::from_toml_str(source)
                .unwrap_or_else(|error| panic!("invalid built-in theme `{stem}`: {error}"))
        })
        .collect()
});

pub fn builtin_themes() -> &'static [ThemeSpec] {
    &BUILTIN
}

pub(crate) fn builtin_theme(slug: &str) -> &'static ThemeSpec {
    BUILTIN
        .iter()
        .find(|spec| spec.slug == slug)
        .unwrap_or_else(|| panic!("missing built-in theme `{slug}`"))
}

pub(crate) fn default_theme() -> &'static ThemeSpec {
    builtin_theme(LIGHT_SLUG)
}
