use crate::cam16::Cam16;
use crate::color::Rgb;
use crate::math::{difference_degrees, rotate_direction, sanitize_degrees};
use crate::solver::solve;

const MOST_HARMONIZING_TURN: f64 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hct {
    hue: f64,
    chroma: f64,
    tone: f64,
    rgb: Rgb,
}

impl Hct {
    #[must_use]
    pub fn of(rgb: Rgb) -> Hct {
        let cam = Cam16::of(rgb);
        Hct {
            hue: cam.hue,
            chroma: cam.chroma,
            tone: rgb.lstar(),
            rgb,
        }
    }

    #[must_use]
    pub fn solve(hue: f64, chroma: f64, tone: f64) -> Hct {
        Hct::of(solve(hue, chroma, tone))
    }

    #[must_use]
    pub const fn hue(self) -> f64 {
        self.hue
    }

    #[must_use]
    pub const fn chroma(self) -> f64 {
        self.chroma
    }

    #[must_use]
    pub const fn tone(self) -> f64 {
        self.tone
    }

    #[must_use]
    pub const fn rgb(self) -> Rgb {
        self.rgb
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TonalPalette {
    hue: f64,
    chroma: f64,
}

impl TonalPalette {
    #[must_use]
    pub const fn new(hue: f64, chroma: f64) -> TonalPalette {
        TonalPalette { hue, chroma }
    }

    #[must_use]
    pub const fn from_hct(hct: Hct) -> TonalPalette {
        TonalPalette::new(hct.hue(), hct.chroma())
    }

    #[must_use]
    pub fn tone(self, tone: f64) -> Rgb {
        solve(self.hue, self.chroma, tone)
    }
}

#[must_use]
pub fn harmonize(design: Rgb, source: Rgb) -> Rgb {
    let from = Hct::of(design);
    let to = Hct::of(source);
    let turn = (difference_degrees(from.hue(), to.hue()) * 0.5).min(MOST_HARMONIZING_TURN);
    let hue = sanitize_degrees(turn.mul_add(rotate_direction(from.hue(), to.hue()), from.hue()));
    solve(hue, from.chroma(), from.tone())
}
