mod chrome;
mod default_browser;
mod engine_form;
mod search;
mod translation;

pub(crate) use chrome::{account_row, view as chrome_view};
pub(crate) use default_browser::view as default_browser_view;
pub(crate) use engine_form::EngineForm;
pub(crate) use search::view as search_view;
pub(crate) use translation::view as translation_view;

use rikka_app::prelude::*;

#[derive(State)]
pub(crate) struct Sections {
    #[state]
    pub(crate) engine: EngineForm,
}

impl Sections {
    pub(crate) fn new() -> Sections {
        Sections {
            engine: EngineForm::new(),
        }
    }
}

impl std::fmt::Debug for Sections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Sections").finish_non_exhaustive()
    }
}
