mod content;
mod default_browser;
mod engine;
mod engine_form;
mod forms;
mod https;
mod languages;
mod network;
mod protection;
mod search;
mod translation;

pub(crate) use content::view as content_view;
pub(crate) use default_browser::view as default_browser_view;
pub(crate) use engine_form::EngineForm;
pub(crate) use forms::EngineForms;
pub(crate) use https::view as https_view;
pub(crate) use network::view as network_view;
pub(crate) use protection::{permissions_view, section_view as protection_view};
pub(crate) use search::view as search_view;
pub(crate) use translation::view as translation_view;

use rikka_app::prelude::*;

#[derive(State)]
pub(crate) struct Sections {
    exceptions: Tracked<u64>,
    #[state]
    pub(crate) engine: EngineForm,
    #[state]
    pub(crate) web: EngineForms,
}

impl Sections {
    pub(crate) fn new() -> Sections {
        Sections {
            exceptions: Tracked::new(0),
            engine: EngineForm::new(),
            web: EngineForms::new(),
        }
    }

    pub(crate) fn exceptions_changed(&mut self) {
        *self.exceptions += 1;
    }

    pub(crate) fn exceptions(&self) -> u64 {
        *self.exceptions
    }
}

impl std::fmt::Debug for Sections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Sections").finish_non_exhaustive()
    }
}
