mod default_browser;
mod downloads;
mod find;
mod home;
mod memory;
mod protection;
mod reading;
mod search;
mod translation;

pub use find::{FindOption, FindOptions};
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

use browser_model::ZOOM_STEPS;

const DOMAIN: &str = "rikka.browser";
const SIDEBAR_EXPANDED: &str = "sidebar-expanded";
const ROUNDED_CORNERS: &str = "rounded-corners";
const ARCHIVE_AFTER: &str = "archive-after";
const HISTORY_KEPT: &str = "history-kept";
const DEFAULT_ZOOM: &str = "default-zoom";
const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;

pub const ZOOMS: [i64; ZOOM_STEPS.len()] = percents(ZOOM_STEPS);

const fn percents(steps: [f64; ZOOM_STEPS.len()]) -> [i64; ZOOM_STEPS.len()] {
    let mut percents = [0; ZOOM_STEPS.len()];
    let mut at = 0;
    while at < steps.len() {
        percents[at] = (steps[at] * 100.0 + 0.5) as i64;
        at += 1;
    }
    percents
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ArchiveAfter {
    #[default]
    Never,
    HalfDay,
    Day,
    Week,
    Month,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HistoryKept {
    Forever,
    Year,
    #[default]
    Quarter,
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

impl Choice for HistoryKept {
    const ALL: &'static [HistoryKept] = &[
        HistoryKept::Forever,
        HistoryKept::Year,
        HistoryKept::Quarter,
        HistoryKept::Month,
    ];

    fn slug(self) -> &'static str {
        match self {
            HistoryKept::Forever => "forever",
            HistoryKept::Year => "365d",
            HistoryKept::Quarter => "90d",
            HistoryKept::Month => "30d",
        }
    }
}

impl HistoryKept {
    pub fn span(self) -> Option<Duration> {
        let days = match self {
            HistoryKept::Forever => return None,
            HistoryKept::Year => 365,
            HistoryKept::Quarter => 90,
            HistoryKept::Month => 30,
        };
        Some(Duration::from_secs(days * DAY))
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

    pub fn history_kept(&self) -> HistoryKept {
        self.observers.read();
        self.settings.borrow().choice(HISTORY_KEPT)
    }

    pub fn set_history_kept(&self, retention: HistoryKept) {
        self.write(|settings| settings.set_choice(HISTORY_KEPT, retention));
    }

    pub fn default_zoom(&self) -> f64 {
        self.observers.read();
        let percent: i64 = self.settings.borrow().get(DEFAULT_ZOOM);
        match ZOOMS.contains(&percent) {
            true => percent as f64 / 100.0,
            false => 1.0,
        }
    }

    pub fn set_default_zoom(&self, percent: i64) {
        self.put::<i64>(DEFAULT_ZOOM, percent);
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
        let schema = reading::keys(protection::keys(search::keys(schema)));
        let schema = downloads::keys(schema);
        let schema = memory::keys(schema);
        let schema = home::keys(schema);
        let schema = default_browser::keys(schema);
        let schema = translation::keys(schema);
        let schema = find::keys(schema);
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
            .key(Key::choice::<HistoryKept>(HISTORY_KEPT).summary("Keep browsing history for"))
            .key(
                Key::int(DEFAULT_ZOOM, 100)
                    .choices(ZOOMS)
                    .summary("Default page zoom in percent"),
            )
    }
}
