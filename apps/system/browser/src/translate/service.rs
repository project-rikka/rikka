use std::cell::{OnceCell, Ref, RefCell, RefMut};

use browser_translators::Translator;

use crate::browser::Browser;
use crate::translate::failure::TranslateFailure;
use crate::translate::tabs::Tabs;

#[derive(Default)]
pub(crate) struct Translations {
    translator: OnceCell<Result<Translator, TranslateFailure>>,
    tabs: RefCell<Tabs>,
}

impl Translations {
    pub(crate) fn translator(&self) -> Result<Translator, TranslateFailure> {
        self.translator
            .get_or_init(|| {
                Translator::new()
                    .inspect_err(|error| tracing::warn!(%error, "cannot create translator"))
                    .map_err(|error| TranslateFailure::from_error(&error))
            })
            .clone()
    }

    pub(crate) fn tabs(&self) -> Ref<'_, Tabs> {
        self.tabs.borrow()
    }

    pub(crate) fn tabs_mut(&self) -> RefMut<'_, Tabs> {
        self.tabs.borrow_mut()
    }
}

impl Browser {
    pub(crate) fn translations(&self) -> &Translations {
        &self.services().translations
    }
}

impl std::fmt::Debug for Translations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Translations")
            .field("opened", &self.translator.get().is_some())
            .finish_non_exhaustive()
    }
}
