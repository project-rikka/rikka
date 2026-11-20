use rikka_omnibox::{Intent, Search, SearchEngines, classify};
use rikka_ui::{MaterialSymbol, tr};
use shell_kit::launch::Spawn;

use crate::provider::{Activation, CERTAIN, Hit, LAST_RESORT, Provider, ProviderInfo, Query};

pub(crate) const ID: &str = "web";

const TRIGGER: &str = "?";

const ABOUT: &str = "about";

#[derive(Debug)]
pub(super) struct WebProvider {
    engines: SearchEngines,
}

impl Default for WebProvider {
    fn default() -> WebProvider {
        WebProvider {
            engines: SearchEngines::builtin(),
        }
    }
}

impl Provider for WebProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo::new(ID, tr!("shell.launcher.provider.web"))
            .icon(MaterialSymbol::TRAVEL_EXPLORE)
            .trigger(TRIGGER)
            .without_history()
    }

    fn search(&mut self, query: &Query) -> Vec<Hit> {
        let text = query.text();
        if text.is_empty() {
            return Vec::new();
        }
        if query.is_forced() {
            return vec![searching(&self.engines.default_engine().search(text), text)];
        }
        match classify(text, &self.engines).intent() {
            Intent::Empty => Vec::new(),
            Intent::Navigate(destination) if destination.url().scheme() != ABOUT => {
                vec![
                    Hit::new(destination.url().as_str(), text)
                        .subtitle(tr!("shell.launcher.web.open"))
                        .icon(MaterialSymbol::PUBLIC)
                        .score(CERTAIN),
                ]
            }
            Intent::Navigate(_) => Vec::new(),
            Intent::Search(search) => vec![searching(search, text)],
        }
    }

    fn activate(&mut self, hit: &Hit) -> Activation {
        Activation::spawn(Spawn::new(rikka_open::opener(hit.id())))
    }
}

fn searching(search: &Search, text: &str) -> Hit {
    Hit::new(search.url().as_str(), text)
        .subtitle(tr!("shell.launcher.web.search"))
        .score(LAST_RESORT)
}
