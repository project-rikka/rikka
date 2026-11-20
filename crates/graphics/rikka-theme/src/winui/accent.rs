use rikka_types::Color;

use crate::tone;
use crate::winui::tokens::argb;

const REGISTRY_ENTRY: usize = 4;
const MAX_TONE: f32 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccentPalette {
    pub accent: Color,
    pub light1: Color,
    pub light2: Color,
    pub light3: Color,
    pub dark1: Color,
    pub dark2: Color,
    pub dark3: Color,
}

impl AccentPalette {
    pub const WINDOWS_DEFAULT: AccentPalette = AccentPalette {
        accent: argb(0xFF0078D4),
        light1: argb(0xFF0091F8),
        light2: argb(0xFF4CC2FF),
        light3: argb(0xFF99EBFF),
        dark1: argb(0xFF0067C0),
        dark2: argb(0xFF003E92),
        dark3: argb(0xFF001A68),
    };

    pub fn from_accent(accent: Color) -> AccentPalette {
        let accent = accent.with_alpha(1.0);
        let reference = AccentPalette::WINDOWS_DEFAULT;
        let origin = tone::measure(reference.accent);
        let own = tone::measure(accent);
        let step = |shade: Color| {
            let target = tone::measure(shade);
            let tone = match target > origin {
                true => own + (MAX_TONE - own) * (target - origin) / (MAX_TONE - origin),
                false => own * target / origin,
            };
            tone::shifted(accent, tone, |chroma| chroma)
        };
        AccentPalette {
            accent,
            light1: step(reference.light1),
            light2: step(reference.light2),
            light3: step(reference.light3),
            dark1: step(reference.dark1),
            dark2: step(reference.dark2),
            dark3: step(reference.dark3),
        }
    }

    #[must_use]
    pub fn from_registry(bytes: &[u8]) -> Option<AccentPalette> {
        let shade = |index: usize| {
            let entry = bytes.get(index * REGISTRY_ENTRY..(index + 1) * REGISTRY_ENTRY)?;
            Some(Color::rgb8(entry[0], entry[1], entry[2]))
        };
        Some(AccentPalette {
            light3: shade(0)?,
            light2: shade(1)?,
            light1: shade(2)?,
            accent: shade(3)?,
            dark1: shade(4)?,
            dark2: shade(5)?,
            dark3: shade(6)?,
        })
    }
}

impl Default for AccentPalette {
    fn default() -> AccentPalette {
        AccentPalette::WINDOWS_DEFAULT
    }
}

#[must_use]
pub fn from_colorref(value: u32) -> Color {
    Color::rgb8(value as u8, (value >> 8) as u8, (value >> 16) as u8)
}
