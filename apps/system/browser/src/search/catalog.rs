use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use rikka_omnibox::{SearchEngine, SearchEngines, Suggester};

use crate::preferences::{CustomEngine, Preferences};

#[derive(Debug, Default)]
pub(crate) struct Searching {
    engines: RefCell<Option<Rc<SearchEngines>>>,
    suggester: OnceCell<Option<Suggester>>,
}

impl Searching {
    pub(crate) fn engines(&self, preferences: &Preferences) -> Rc<SearchEngines> {
        if let Some(engines) = &*self.engines.borrow() {
            return Rc::clone(engines);
        }
        let engines = Rc::new(engines_of(
            &preferences.custom_engines(),
            &preferences.search_engine(),
        ));
        *self.engines.borrow_mut() = Some(Rc::clone(&engines));
        engines
    }

    pub(crate) fn forget(&self) {
        self.engines.borrow_mut().take();
    }

    pub(crate) fn suggester(&self) -> Option<Suggester> {
        self.suggester
            .get_or_init(|| {
                let language = rikka_i18n::current_locale().to_string();
                Suggester::builder()
                    .accept_language(format!("{language},en;q=0.8"))
                    .build()
                    .inspect_err(|error| tracing::warn!(%error, "cannot create suggestion client"))
                    .ok()
            })
            .clone()
    }
}

pub(crate) fn engines_of(custom: &[CustomEngine], default: &str) -> SearchEngines {
    let mut engines = SearchEngines::builtin();
    for custom_engine in custom {
        let added = engine_of(custom_engine).and_then(|engine| engines.add(engine));
        if let Err(error) = added {
            tracing::warn!(%error, name = %custom_engine.name, "skipping custom search engine");
        }
    }
    if engines.set_default(default).is_err() {
        tracing::debug!(%default, "unknown default search engine");
    }
    engines
}

pub(crate) fn engine_of(custom_engine: &CustomEngine) -> rikka_omnibox::Result<SearchEngine> {
    let builder = SearchEngine::builder(custom_engine.id.as_str())
        .name(custom_engine.name.as_str())
        .keyword(custom_engine.keyword.as_str())
        .search_url(custom_engine.url.as_str());
    match custom_engine.suggestions.trim() {
        "" => builder.build(),
        suggestions => builder.suggestion_url(suggestions).build(),
    }
}
