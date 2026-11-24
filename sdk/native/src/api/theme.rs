use std::sync::Arc;

use rikka_appearance::Appearance;
use rikka_theme::{MaterialYou, Palette, Theme, ThemeRegistry, ThemeSpec};

use crate::abi::violation;
use crate::ffi;
use crate::values::palette_color;

#[derive(Debug)]
struct Entry {
    slug: String,
    name: String,
    dark: bool,
    palette: Palette,
}

#[derive(Debug)]
pub(crate) struct ThemeCatalog {
    entries: Vec<Entry>,
}

impl ThemeCatalog {
    fn entry(&self, index: usize) -> &Entry {
        self.entries.get(index).unwrap_or_else(|| {
            violation(format!(
                "theme {index} is past the {} installed",
                self.entries.len()
            ))
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Family {
    Rikka,
    Dynamic,
    Installed,
}

impl Family {
    fn of(spec: &ThemeSpec) -> Self {
        match spec.slug.as_str() {
            Theme::LIGHT_SLUG | Theme::DARK_SLUG => Self::Rikka,
            MaterialYou::DARK_SLUG | MaterialYou::LIGHT_SLUG => Self::Dynamic,
            _ => Self::Installed,
        }
    }
}

pub(crate) fn theme_catalog_new() -> Arc<ThemeCatalog> {
    let registry = Appearance::current().registry().unwrap_or_else(|error| {
        tracing::warn!("failed to read installed themes: {error}");
        ThemeRegistry::embedded()
    });
    let mut specs: Vec<&ThemeSpec> = registry.iter().collect();
    specs.sort_by_key(|spec| Family::of(spec));
    let entries = specs
        .into_iter()
        .map(|spec| {
            let theme = Theme::from_spec(spec);
            Entry {
                slug: spec.slug.clone(),
                name: spec.name.clone(),
                dark: theme.is_dark(),
                palette: theme.palette(),
            }
        })
        .collect();
    Arc::new(ThemeCatalog { entries })
}

pub(crate) fn theme_catalog_count(catalog: &ThemeCatalog) -> usize {
    catalog.entries.len()
}

pub(crate) fn theme_catalog_slug(catalog: &ThemeCatalog, index: usize) -> String {
    catalog.entry(index).slug.clone()
}

pub(crate) fn theme_catalog_name(catalog: &ThemeCatalog, index: usize) -> String {
    catalog.entry(index).name.clone()
}

pub(crate) fn theme_catalog_is_dark(catalog: &ThemeCatalog, index: usize) -> bool {
    catalog.entry(index).dark
}

pub(crate) fn theme_catalog_color(
    catalog: &ThemeCatalog,
    index: usize,
    role: ffi::ColorRole,
) -> ffi::Color {
    palette_color(&catalog.entry(index).palette, role)
}
