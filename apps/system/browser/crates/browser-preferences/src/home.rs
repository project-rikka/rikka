use rikka_settings::{Key, Schema};

use crate::Preferences;

const HIDDEN_SITES: &str = "home-hidden-sites";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HomePart {
    Clock,
    Search,
    TopSites,
    Favorites,
    Closed,
}

impl HomePart {
    pub const ALL: [HomePart; 5] = [
        HomePart::Clock,
        HomePart::Search,
        HomePart::TopSites,
        HomePart::Favorites,
        HomePart::Closed,
    ];

    fn key(self) -> &'static str {
        match self {
            HomePart::Clock => "home-clock",
            HomePart::Search => "home-search",
            HomePart::TopSites => "home-top-sites",
            HomePart::Favorites => "home-favorites",
            HomePart::Closed => "home-recently-closed",
        }
    }

    fn summary(self) -> &'static str {
        match self {
            HomePart::Clock => "Show the clock and date on Home",
            HomePart::Search => "Show the search box on Home",
            HomePart::TopSites => "Show top sites on Home",
            HomePart::Favorites => "Show favorites on Home",
            HomePart::Closed => "Show recently closed tabs on Home",
        }
    }
}

impl Preferences {
    pub fn home_shows(&self, part: HomePart) -> bool {
        self.observers.read();
        self.settings.borrow().get(part.key())
    }

    pub fn set_home_shows(&self, part: HomePart, shown: bool) {
        self.put::<bool>(part.key(), shown);
    }

    pub fn hidden_sites(&self) -> Vec<String> {
        self.observers.read();
        self.settings.borrow().get::<Vec<String>>(HIDDEN_SITES)
    }

    pub fn hide_site(&self, url: &str) {
        let mut hidden = self.hidden_sites();
        if !hidden.iter().any(|known| known == url) {
            hidden.push(url.to_owned());
            self.put::<Vec<String>>(HIDDEN_SITES, hidden);
        }
    }

    pub fn show_hidden_sites(&self) {
        self.put::<Vec<String>>(HIDDEN_SITES, Vec::new());
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    HomePart::ALL
        .into_iter()
        .fold(schema, |schema, part| {
            schema.key(Key::bool(part.key(), true).summary(part.summary()))
        })
        .key(Key::list(HIDDEN_SITES, []).summary("Addresses hidden from top sites on Home"))
}
