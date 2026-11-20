use rikka_app::prelude::*;
use rikka_input::{Layout, Layouts};

pub(crate) fn layouts(layouts: &Layouts) -> Vec<(String, Label)> {
    layouts
        .all()
        .iter()
        .map(|layout| {
            (
                layout.name.clone(),
                Label::from(layout.description.as_str()),
            )
        })
        .collect()
}

pub(crate) fn variants(layout: Option<&Layout>) -> Vec<(String, Label)> {
    layout
        .map(|layout| {
            layout
                .variants
                .iter()
                .map(|variant| {
                    (
                        variant.name.clone(),
                        Label::from(variant.description.as_str()),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn named(keymaps: &[String]) -> Vec<(String, Label)> {
    keymaps
        .iter()
        .map(|keymap| (keymap.clone(), Label::from(keymap.as_str())))
        .collect()
}
