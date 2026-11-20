use rikka_llm_accounts::{ModelOptions, Row};
use rikka_ui::Label;
use rikka_widgets::{MenuItem, menu_header, menu_item, menu_separator};

#[must_use]
pub(crate) fn names(offering: &ModelOptions) -> Vec<Label> {
    offering
        .options()
        .iter()
        .map(|pick| Label::from(pick.name().to_owned()))
        .collect()
}

#[must_use]
pub(crate) fn rows(offering: &ModelOptions) -> Vec<MenuItem> {
    offering
        .rows()
        .iter()
        .map(|row| match row {
            Row::Account(at) => menu_header(offering.options()[*at].account()),
            Row::Model(at) => menu_item(offering.options()[*at].name())
                .selected(offering.selected_index() == Some(*at)),
            Row::Separator | Row::Levels => menu_separator(),
        })
        .collect()
}
