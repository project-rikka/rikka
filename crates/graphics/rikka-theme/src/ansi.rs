use rikka_types::Color;

use crate::tone;
use crate::variant::Variant;

const BRIGHT_STEP: f32 = 12.0;

const HUE_CEILING: f32 = 88.0;
const HUE_FLOOR: f32 = 26.0;

const VIVID: f64 = 0.35;

const BLACK_STEP: f32 = 26.0;
const BLACK_CEILING: f32 = 30.0;

pub(crate) const BLACK: usize = 0;
pub(crate) const RED: usize = 1;
pub(crate) const GREEN: usize = 2;
pub(crate) const YELLOW: usize = 3;
pub(crate) const BLUE: usize = 4;
pub(crate) const MAGENTA: usize = 5;
pub(crate) const CYAN: usize = 6;
pub(crate) const WHITE: usize = 7;

const BRIGHT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ansi {
    pub colors: [Color; 16],
    pub foreground: Color,
    pub background: Color,
    pub cursor: Color,
}
pub(crate) struct Hues {
    pub(crate) red: Color,
    pub(crate) orange: Color,
    pub(crate) green: Color,
    pub(crate) yellow: Color,
    pub(crate) blue: Color,
    pub(crate) magenta: Color,
    pub(crate) purple: Color,
    pub(crate) cyan: Color,
}

pub(crate) struct Ink {
    pub(crate) background: Color,
    pub(crate) text: Color,
    pub(crate) subtext: Color,
    pub(crate) muted: Color,
    pub(crate) cursor: Color,
}

impl Ansi {
    pub(crate) fn derive(hues: &Hues, ink: &Ink, variant: Variant) -> Ansi {
        let (step, floor, ceiling) = if variant.is_dark() {
            (BRIGHT_STEP, 0.0, HUE_CEILING)
        } else {
            (-BRIGHT_STEP, HUE_FLOOR, 100.0)
        };
        let vivid = |color: Color| {
            let target_tone = tone::measure(color) + step;
            let lifted = target_tone.clamp(floor, ceiling);
            let unspent = f64::from(((target_tone - lifted) / BRIGHT_STEP).abs().min(1.0));
            tone::shifted(color, lifted, |chroma| chroma * (1.0 + VIVID * unspent))
        };

        let mut colors = [tone::with_tone(
            ink.muted,
            (tone::measure(ink.muted) - BLACK_STEP).min(BLACK_CEILING),
        ); 16];
        colors[RED] = hues.red;
        colors[GREEN] = hues.green;
        colors[YELLOW] = hues.yellow;
        colors[BLUE] = hues.blue;
        colors[MAGENTA] = hues.magenta;
        colors[CYAN] = hues.cyan;
        colors[WHITE] = ink.subtext;
        colors[BRIGHT + BLACK] = ink.muted;
        for hue in RED..=CYAN {
            colors[BRIGHT + hue] = vivid(colors[hue]);
        }
        colors[BRIGHT + WHITE] = tone::with_tone(ink.text, tone::measure(ink.text) + step);

        Ansi {
            colors,
            foreground: ink.text,
            background: ink.background,
            cursor: ink.cursor,
        }
    }
}
