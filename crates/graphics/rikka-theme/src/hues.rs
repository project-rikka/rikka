use rikka_material_color::harmonize;
use rikka_types::Color;

use crate::ansi::Hues;
use crate::tone::{self, from_rgb, rgb};
use crate::variant::Variant;

const TONE_DARK: f32 = 80.0;
const TONE_LIGHT: f32 = 40.0;

const RED: u32 = 0xf44336;
const ORANGE: u32 = 0xff9800;
const YELLOW: u32 = 0xffc107;
const GREEN: u32 = 0x4caf50;
const CYAN: u32 = 0x00bcd4;
const BLUE: u32 = 0x2196f3;
const PURPLE: u32 = 0x9c27b0;
const MAGENTA: u32 = 0xe91e63;

pub(crate) fn harmonized(source: Color, variant: Variant) -> Hues {
    let tone = match variant {
        Variant::Dark => TONE_DARK,
        Variant::Light => TONE_LIGHT,
    };
    let source = rgb(source);
    let hue =
        |design: u32| tone::with_tone(from_rgb(harmonize(rgb(Color::hex(design)), source)), tone);
    Hues {
        red: hue(RED),
        orange: hue(ORANGE),
        green: hue(GREEN),
        yellow: hue(YELLOW),
        blue: hue(BLUE),
        magenta: hue(MAGENTA),
        purple: hue(PURPLE),
        cyan: hue(CYAN),
    }
}
