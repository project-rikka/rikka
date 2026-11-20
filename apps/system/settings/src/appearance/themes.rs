use rikka_app::prelude::*;
use rikka_theme::{MaterialYou, Theme, ThemeRegistry, ThemeSpec, Variant};

use crate::appearance::component::Appearance;

const TRIGGER: &str = "settings-theme";

pub(super) struct Installed {
    pub(super) slug: String,
    name: String,
    category: Category,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Category {
    Rikka,
    Dark,
    Light,
    Dynamic,
}

impl Category {
    fn from_spec(spec: &ThemeSpec) -> Self {
        match spec.slug.as_str() {
            Theme::LIGHT_SLUG | Theme::DARK_SLUG => Self::Rikka,
            MaterialYou::DARK_SLUG | MaterialYou::LIGHT_SLUG => Self::Dynamic,
            _ => match spec.variant {
                Variant::Dark => Self::Dark,
                Variant::Light => Self::Light,
            },
        }
    }

    fn label(self) -> Label {
        match self {
            Self::Rikka => tr!("settings.appearance.categories.rikka"),
            Self::Dark => tr!("settings.appearance.categories.dark"),
            Self::Light => tr!("settings.appearance.categories.light"),
            Self::Dynamic => tr!("settings.appearance.categories.dynamic"),
        }
    }
}

pub(super) fn installed(registry: Option<&ThemeRegistry>) -> Vec<Installed> {
    let mut themes: Vec<Installed> = registry
        .into_iter()
        .flat_map(ThemeRegistry::iter)
        .map(|spec| Installed {
            slug: spec.slug.clone(),
            name: spec.name.clone(),
            category: Category::from_spec(spec),
        })
        .collect();
    themes.sort_by_key(|theme| theme.category);
    themes
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ThemeChoice {
    Default,
    Category,
    Theme(usize),
}

pub(super) fn trigger(themes: &[Installed], selected: Option<usize>, open: bool) -> Element {
    dropdown(Some(selected.map_or(0, |index| index + 1)))
        .options(
            std::iter::once(tr!("settings.appearance.default"))
                .chain(themes.iter().map(|theme| Label::from(theme.name.as_str()))),
        )
        .open(open)
        .on_toggle(|appearance: &mut Appearance, open: bool| appearance.set_theme_menu_open(open))
        .on_select(|appearance: &mut Appearance, index: usize| {
            appearance.pick(
                index
                    .checked_sub(1)
                    .map_or(ThemeChoice::Default, ThemeChoice::Theme),
            );
        })
        .width(220.0)
        .build()
        .keyed(TRIGGER)
}

fn rows(themes: &[Installed], selected: Option<usize>) -> (Vec<MenuItem>, Vec<ThemeChoice>) {
    let mut items =
        vec![menu_item(tr!("settings.appearance.default")).selected(selected.is_none())];
    let mut picks = vec![ThemeChoice::Default];
    let mut listed = None;
    for (index, theme) in themes.iter().enumerate() {
        if listed != Some(theme.category) {
            items.push(menu_header(theme.category.label()));
            picks.push(ThemeChoice::Category);
            listed = Some(theme.category);
        }
        items.push(menu_item(theme.name.as_str()).selected(selected == Some(index)));
        picks.push(ThemeChoice::Theme(index));
    }
    (items, picks)
}

pub(super) fn panel(themes: &[Installed], selected: Option<usize>) -> Element {
    let (items, picks) = rows(themes, selected);
    menu()
        .items(items)
        .on_activate(move |appearance: &mut Appearance, row: usize| appearance.pick(picks[row]))
        .on_dismiss(|appearance: &mut Appearance| appearance.set_theme_menu_open(false))
        .build()
        .anchored(Anchor::below(TRIGGER).offset(0.0, 4.0).match_target_width())
}
