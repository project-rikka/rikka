mod catalog;

pub(crate) use catalog::{Searching, engine_of};

use std::rc::Rc;

use browser_preferences::CustomEngine;
use rikka_omnibox::{SearchEngines, Suggester};

use crate::browser::Browser;

impl Browser {
    pub(crate) fn search_engines(&self) -> Rc<SearchEngines> {
        self.services().search.engines(self.preferences())
    }

    pub(crate) fn search_url(&self, terms: &str) -> String {
        self.search_engines()
            .default_engine()
            .search_url(terms)
            .into()
    }

    pub(crate) fn suggester(&self) -> Option<Suggester> {
        self.services().search.suggester()
    }

    pub(crate) fn set_default_engine(&self, id: &str) {
        self.preferences().set_search_engine(id);
        self.services().search.forget();
    }

    pub(crate) fn save_custom_engines(&self, engines: &[CustomEngine]) {
        self.preferences().set_custom_engines(engines);
        self.services().search.forget();
    }
}
