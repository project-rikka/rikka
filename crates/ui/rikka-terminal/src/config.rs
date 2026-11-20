use std::rc::Rc;

use rikka_settings::seam::Preferences;
use rikka_settings::{Key, Schema};

use crate::clipboard::{ClipboardAccess, LIMIT};
use crate::font::Font;
use crate::options::{self, Options};

const DOMAIN: &str = "rikka.terminal";

const FONT_FAMILY: &str = "font-family";
const FONT_SIZE: &str = "font-size";
const SCROLLBACK: &str = "scrollback";
const CLIPBOARD_READ: &str = "clipboard-read";
const CLIPBOARD_WRITE: &str = "clipboard-write";
const CLIPBOARD_LIMIT: &str = "clipboard-max-size";

const DEFAULT_FAMILY: &str = "monospace";
const DEFAULT_SIZE: i64 = 13;
const SIZE_RANGE: std::ops::RangeInclusive<i64> = 6..=48;
const DEFAULT_SCROLLBACK: i64 = options::SCROLLBACK as i64;
const MEBIBYTE: usize = 1 << 20;
const DEFAULT_LIMIT: i64 = (LIMIT / MEBIBYTE) as i64;
const LIMIT_RANGE: std::ops::RangeInclusive<i64> = 0..=1024;
const ALLOWED: bool = true;

#[derive(Debug)]
pub struct Config {
    preferences: Preferences,
    family: Rc<str>,
    size: f32,
    scrollback: usize,
    access: ClipboardAccess,
    unsaved: bool,
}

impl Config {
    pub fn load() -> Self {
        Self::from_preferences(Preferences::open(schema()))
    }

    fn from_preferences(preferences: Preferences) -> Self {
        Self {
            family: Rc::from(preferences.get::<String>(FONT_FAMILY)),
            size: preferences.get::<i64>(FONT_SIZE) as f32,
            scrollback: preferences.get::<i64>(SCROLLBACK).max(0) as usize,
            access: access(&preferences),
            preferences,
            unsaved: false,
        }
    }

    pub fn font(&self) -> Font {
        Font::new(&self.family, self.size)
    }

    pub fn options(&self) -> Options {
        Options::new()
            .scrollback(self.scrollback)
            .clipboard(self.access)
    }

    pub fn resize_font(&mut self, steps: i32) {
        let size =
            (self.size as i64 + i64::from(steps)).clamp(*SIZE_RANGE.start(), *SIZE_RANGE.end());
        self.set_size(size);
    }

    pub fn reset_font(&mut self) {
        self.set_size(DEFAULT_SIZE);
    }

    pub fn save_font_size(&mut self) {
        if !self.unsaved {
            return;
        }
        self.unsaved = !self.preferences.set(FONT_SIZE, self.size as i64);
    }

    fn set_size(&mut self, size: i64) {
        if size as f32 == self.size {
            return;
        }
        self.size = size as f32;
        self.unsaved = true;
    }
}

fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("Terminal")
        .key(
            Key::string(FONT_FAMILY, DEFAULT_FAMILY)
                .summary("The fixed-pitch family the grid is drawn in"),
        )
        .key(
            Key::int(FONT_SIZE, DEFAULT_SIZE)
                .range(SIZE_RANGE)
                .summary("How big the text is, in logical pixels"),
        )
        .key(
            Key::int(SCROLLBACK, DEFAULT_SCROLLBACK)
                .range(0..=1_000_000)
                .summary("How many lines that have scrolled off the top are kept"),
        )
        .key(
            Key::bool(CLIPBOARD_READ, false)
                .summary("Whether a program may read what the selections hold"),
        )
        .key(
            Key::bool(CLIPBOARD_WRITE, ALLOWED)
                .summary("Whether a program may put anything on the selections"),
        )
        .key(
            Key::int(CLIPBOARD_LIMIT, DEFAULT_LIMIT)
                .range(LIMIT_RANGE)
                .summary(
                    "How much one clipboard sequence may carry, in mebibytes; 0 is any amount",
                ),
        )
}

fn access(preferences: &Preferences) -> ClipboardAccess {
    let limit = preferences
        .get::<i64>(CLIPBOARD_LIMIT)
        .clamp(0, *LIMIT_RANGE.end());
    ClipboardAccess::new()
        .reading(preferences.get(CLIPBOARD_READ))
        .writing(preferences.get(CLIPBOARD_WRITE))
        .limit((limit > 0).then(|| limit as usize * MEBIBYTE))
}
