mod default_browser;
mod home;
mod memory;
mod reading;
mod search;
mod translation;

pub use home::HomePart;
pub use memory::MemorySaver;
pub use reading::{ReaderStyle, ReaderTheme};
pub use search::CustomEngine;
pub use translation::{Rule, TranslationRules, session_language};

use std::cell::RefCell;
use std::time::Duration;

use rikka_settings::{Key, Schema};
use rikka_types::Choice;
use rikka_ui::Observers;

const DOMAIN: &str = "rikka.browser";
const SIDEBAR_EXPANDED: &str = "sidebar-expanded";
const ROUNDED_CORNERS: &str = "rounded-corners";
const ARCHIVE_AFTER: &str = "archive-after";
const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ArchiveAfter {
    #[default]
    Never,
    HalfDay,
    Day,
    Week,
    Month,
}

impl Choice for ArchiveAfter {
    const ALL: &'static [ArchiveAfter] = &[
        ArchiveAfter::Never,
        ArchiveAfter::HalfDay,
        ArchiveAfter::Day,
        ArchiveAfter::Week,
        ArchiveAfter::Month,
    ];

    fn slug(self) -> &'static str {
        match self {
            ArchiveAfter::Never => "never",
            ArchiveAfter::HalfDay => "12h",
            ArchiveAfter::Day => "24h",
            ArchiveAfter::Week => "7d",
            ArchiveAfter::Month => "30d",
        }
    }
}

impl ArchiveAfter {
    pub fn idle(self) -> Option<Duration> {
        let seconds = match self {
            ArchiveAfter::Never => return None,
            ArchiveAfter::HalfDay => 12 * HOUR,
            ArchiveAfter::Day => DAY,
            ArchiveAfter::Week => 7 * DAY,
            ArchiveAfter::Month => 30 * DAY,
        };
        Some(Duration::from_secs(seconds))
    }
}

#[derive(Debug)]
pub struct Preferences {
    settings: RefCell<rikka_settings::seam::Preferences>,
    observers: Observers,
}

impl Preferences {
    #[must_use]
    pub fn open() -> Preferences {
        Preferences::from_seam(rikka_settings::seam::Preferences::open(Self::schema()))
    }

    fn from_seam(settings: rikka_settings::seam::Preferences) -> Preferences {
        Preferences {
            settings: RefCell::new(settings),
            observers: Observers::new(),
        }
    }

    pub fn sidebar_expanded(&self) -> bool {
        self.observers.read();
        self.settings.borrow().get(SIDEBAR_EXPANDED)
    }

    pub fn set_sidebar_expanded(&self, expanded: bool) {
        self.put::<bool>(SIDEBAR_EXPANDED, expanded);
    }

    pub fn rounded_corners(&self) -> bool {
        self.observers.read();
        self.settings.borrow().get(ROUNDED_CORNERS)
    }

    pub fn set_rounded_corners(&self, rounded: bool) {
        self.put::<bool>(ROUNDED_CORNERS, rounded);
    }

    pub fn archive_after(&self) -> ArchiveAfter {
        self.observers.read();
        self.settings.borrow().choice(ARCHIVE_AFTER)
    }

    pub fn set_archive_after(&self, after: ArchiveAfter) {
        self.write(|settings| settings.set_choice(ARCHIVE_AFTER, after));
    }

    fn put<T>(&self, key: &str, value: T)
    where
        T: PartialEq + serde::Serialize + serde::de::DeserializeOwned + Default,
    {
        self.write(|settings| {
            if settings.get::<T>(key) == value {
                return false;
            }
            settings.set(key, value);
            true
        });
    }

    fn write(&self, apply: impl FnOnce(&mut rikka_settings::seam::Preferences) -> bool) {
        if apply(&mut self.settings.borrow_mut()) {
            self.observers.changed();
        }
    }
}

impl Default for Preferences {
    fn default() -> Preferences {
        Preferences::from_seam(rikka_settings::seam::Preferences::in_memory(Self::schema()))
    }
}

impl Preferences {
    #[must_use]
    pub fn schema() -> Schema {
        let schema = Schema::new(DOMAIN);
        let schema = reading::keys(search::keys(schema));
        let schema = memory::keys(schema);
        let schema = home::keys(schema);
        let schema = default_browser::keys(schema);
        let schema = translation::keys(schema);
        schema
            .summary("Browser")
            .key(Key::bool(SIDEBAR_EXPANDED, true).summary("Show the sidebar expanded with titles"))
            .key(
                Key::bool(ROUNDED_CORNERS, true)
                    .summary("Round the page, sidebar and address field corners"),
            )
            .key(
                Key::choice::<ArchiveAfter>(ARCHIVE_AFTER)
                    .summary("Archive Today tabs left unused for"),
            )
    }
}
