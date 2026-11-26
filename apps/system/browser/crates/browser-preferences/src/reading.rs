use browser_reader::{FontSize, Spacing, Typeface, Width};
use rikka_settings::{Key, Schema};
use rikka_types::Choice;

use crate::Preferences;

const TYPEFACE: &str = "reader-typeface";
const SIZE: &str = "reader-size";
const WIDTH: &str = "reader-width";
const SPACING: &str = "reader-spacing";
const THEME: &str = "reader-theme";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ReaderTheme {
    #[default]
    Auto,
    Light,
    Sepia,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReaderStyle {
    pub typeface: Typeface,
    pub size: FontSize,
    pub width: Width,
    pub spacing: Spacing,
    pub theme: ReaderTheme,
}

impl Choice for ReaderTheme {
    const ALL: &'static [ReaderTheme] = &[
        ReaderTheme::Auto,
        ReaderTheme::Light,
        ReaderTheme::Sepia,
        ReaderTheme::Dark,
    ];

    fn slug(self) -> &'static str {
        match self {
            ReaderTheme::Auto => "auto",
            ReaderTheme::Light => "light",
            ReaderTheme::Sepia => "sepia",
            ReaderTheme::Dark => "dark",
        }
    }
}

impl Preferences {
    pub fn reader_style(&self) -> ReaderStyle {
        self.observers.read();
        let settings = self.settings.borrow();
        let size: i64 = settings.get(SIZE);
        ReaderStyle {
            typeface: settings.choice(TYPEFACE),
            size: FontSize::step(u8::try_from(size).unwrap_or(FontSize::default().index())),
            width: settings.choice(WIDTH),
            spacing: settings.choice(SPACING),
            theme: settings.choice(THEME),
        }
    }

    pub fn set_reader_style(&self, style: ReaderStyle) {
        self.write(|settings| {
            let mut changed = settings.set_choice(TYPEFACE, style.typeface);
            changed |= settings.set_choice(WIDTH, style.width);
            changed |= settings.set_choice(SPACING, style.spacing);
            changed |= settings.set_choice(THEME, style.theme);
            let size = i64::from(style.size.index());
            if settings.get::<i64>(SIZE) != size {
                settings.set(SIZE, size);
                changed = true;
            }
            changed
        });
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    let style = ReaderStyle::default();
    schema
        .key(Key::choice::<Typeface>(TYPEFACE).summary("Reader typeface"))
        .key(
            Key::int(SIZE, i64::from(style.size.index()))
                .choices((0..=i64::from(FontSize::LARGEST.index())).collect::<Vec<_>>())
                .summary("Reader text size step"),
        )
        .key(Key::choice::<Width>(WIDTH).summary("Reader line width"))
        .key(Key::choice::<Spacing>(SPACING).summary("Reader line spacing"))
        .key(Key::choice::<ReaderTheme>(THEME).summary("Reader colours"))
}
