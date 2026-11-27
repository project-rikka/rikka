mod archive;
mod clearing;
pub(crate) mod days;
mod downloads;
mod history;
mod pages;
pub(crate) mod route;
mod settings;
mod status;

pub(crate) use clearing::Span;
pub(crate) use history::HistoryPage;
pub(crate) use pages::Pages;

use browser_pages::{Internal, Section};
use rikka_app::prelude::*;

use crate::shell::Shell;
use browser_model::TabId;
use route::Presented;

pub(crate) fn view(internal: &Internal, tab: TabId) -> Element {
    match internal {
        Internal::Home => crate::home::view(tab),
        Internal::Settings(Some(Section::ClearData)) => settings::framed(
            Section::ClearData,
            component(|shell: &Shell| shell, clearing::view),
        ),
        Internal::Settings(section) => settings::view(*section),
        Internal::History => component(|page: &HistoryPage| page, history::view).any(),
        Internal::Archive => component(|shell: &Shell| shell, archive::view).any(),
        Internal::Downloads => component(|shell: &Shell| shell, downloads::view).any(),
        Internal::Extensions => component(|shell: &Shell| shell, crate::extensions::page).any(),
        Internal::Missing(_) => status_page(MaterialSymbol::WARNING, internal.title()).any(),
    }
}
