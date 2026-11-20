use rikka_settings::{Key, Schema};

use crate::preferences::Preferences;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum HomePart {
    Clock,
    Search,
    TopSites,
    Favorites,
    Closed,
}

impl HomePart {
    pub(crate) const ALL: [HomePart; 5] = [
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
    pub(crate) fn home_shows(&self, part: HomePart) -> bool {
        self.observers.read();
        self.settings.borrow().get(part.key())
    }

    pub(crate) fn set_home_shows(&self, part: HomePart, shown: bool) {
        self.put::<bool>(part.key(), shown);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    HomePart::ALL.into_iter().fold(schema, |schema, part| {
        schema.key(Key::bool(part.key(), true).summary(part.summary()))
    })
}
