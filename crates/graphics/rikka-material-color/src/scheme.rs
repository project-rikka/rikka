use crate::color::Rgb;
use crate::contrast::{ContrastCurve, foreground_tone, ratio_of_tones};
use crate::hct::{Hct, TonalPalette};
use crate::math::sanitize_degrees;

const ERROR_PALETTE: TonalPalette = TonalPalette::new(25.0, 84.0);
const ACCENT_CURVE: ContrastCurve = ContrastCurve::new(3.0, 4.5, 7.0, 7.0);
const CONTAINER_CURVE: ContrastCurve = ContrastCurve::new(1.0, 1.0, 3.0, 4.5);
const ACCENT_TO_CONTAINER_DELTA: f64 = 10.0;
const AWKWARD_TONES: std::ops::Range<f64> = 50.0..60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    TonalSpot,
    Vibrant,
    Expressive,
    Neutral,
    Monochrome,
    Fidelity,
    Content,
    Rainbow,
    FruitSalad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scheme {
    pub primary: Rgb,
    pub error: Rgb,
    pub surface: Rgb,
    pub surface_container_lowest: Rgb,
    pub surface_container_low: Rgb,
    pub surface_container: Rgb,
    pub surface_container_high: Rgb,
    pub surface_container_highest: Rgb,
    pub on_surface: Rgb,
    pub on_surface_variant: Rgb,
    pub outline: Rgb,
    pub outline_variant: Rgb,
}

struct Palettes {
    primary: TonalPalette,
    neutral: TonalPalette,
    neutral_variant: TonalPalette,
}

impl Palettes {
    fn new(source: Hct, variant: Variant) -> Palettes {
        let (hue, chroma) = (source.hue(), source.chroma());
        let (primary, neutral, neutral_variant) = match variant {
            Variant::TonalSpot => ((hue, 36.0), (hue, 6.0), (hue, 8.0)),
            Variant::Vibrant => ((hue, 200.0), (hue, 10.0), (hue, 10.0)),
            Variant::Expressive => (
                (sanitize_degrees(hue + 240.0), 40.0),
                (hue + 15.0, 8.0),
                (hue + 15.0, 12.0),
            ),
            Variant::Neutral => ((hue, 12.0), (hue, 2.0), (hue, 2.0)),
            Variant::Monochrome => ((hue, 0.0), (hue, 0.0), (hue, 0.0)),
            Variant::Fidelity | Variant::Content => (
                (hue, chroma),
                (hue, chroma / 8.0),
                (hue, (chroma / 8.0) + 4.0),
            ),
            Variant::Rainbow => ((hue, 48.0), (hue, 0.0), (hue, 0.0)),
            Variant::FruitSalad => (
                (sanitize_degrees(hue - 50.0), 48.0),
                (hue, 10.0),
                (hue, 16.0),
            ),
        };
        let palette = |(hue, chroma): (f64, f64)| TonalPalette::new(hue, chroma);
        Palettes {
            primary: palette(primary),
            neutral: palette(neutral),
            neutral_variant: palette(neutral_variant),
        }
    }
}

struct Conditions {
    source: Hct,
    variant: Variant,
    dark: bool,
    contrast: f64,
}

impl Conditions {
    fn by_mode(&self, dark: f64, light: f64) -> f64 {
        if self.dark { dark } else { light }
    }

    fn curved(&self, dark: [f64; 4], light: [f64; 4]) -> f64 {
        let [low, normal, medium, high] = if self.dark { dark } else { light };
        ContrastCurve::new(low, normal, medium, high).at(self.contrast)
    }

    fn highest_surface(&self) -> f64 {
        self.curved([24.0, 24.0, 29.0, 34.0], [87.0, 87.0, 80.0, 75.0])
    }

    fn foreground(&self, initial: f64, curve: ContrastCurve) -> f64 {
        let background = self.highest_surface();
        let desired = curve.at(self.contrast);
        if self.contrast >= 0.0 && ratio_of_tones(background, initial) >= desired {
            initial
        } else {
            foreground_tone(background, desired)
        }
    }

    fn accent(&self, container_initial: f64, accent_initial: f64) -> f64 {
        let background = self.highest_surface();
        let direction = if self.dark { 1.0 } else { -1.0 };
        let against = |initial: f64, curve: ContrastCurve| {
            let desired = curve.at(self.contrast);
            if self.contrast >= 0.0 && ratio_of_tones(background, initial) >= desired {
                initial
            } else {
                foreground_tone(background, desired)
            }
        };
        let delta = ACCENT_TO_CONTAINER_DELTA;
        let mut nearer = against(container_initial, CONTAINER_CURVE);
        let mut farther = against(accent_initial, ACCENT_CURVE);
        let apart = |farther: f64, nearer: f64| (farther - nearer) * direction >= delta;
        if !apart(farther, nearer) {
            farther = delta.mul_add(direction, nearer).clamp(0.0, 100.0);
            if !apart(farther, nearer) {
                nearer = delta.mul_add(-direction, farther).clamp(0.0, 100.0);
            }
        }
        if AWKWARD_TONES.contains(&nearer) {
            if direction > 0.0 {
                farther = farther.max(delta.mul_add(direction, 60.0));
            } else {
                farther = farther.min(delta.mul_add(direction, 49.0));
            }
        } else if AWKWARD_TONES.contains(&farther) {
            farther = if direction > 0.0 { 60.0 } else { 49.0 };
        }
        farther
    }

    fn primary(&self) -> f64 {
        let fidelity = matches!(self.variant, Variant::Fidelity | Variant::Content);
        let monochrome = self.variant == Variant::Monochrome;
        let container = if fidelity {
            self.source.tone()
        } else if monochrome {
            self.by_mode(85.0, 25.0)
        } else {
            self.by_mode(30.0, 90.0)
        };
        let accent = if monochrome {
            self.by_mode(100.0, 0.0)
        } else {
            self.by_mode(80.0, 40.0)
        };
        self.accent(container, accent)
    }
}

impl Scheme {
    #[must_use]
    pub fn new(source: Rgb, variant: Variant, dark: bool, contrast: f64) -> Scheme {
        let source = Hct::of(source);
        let palettes = Palettes::new(source, variant);
        let conditions = Conditions {
            source,
            variant,
            dark,
            contrast,
        };
        let neutral = |tone: f64| palettes.neutral.tone(tone);
        let neutral_variant = |initial: f64, curve: [f64; 4]| {
            let [low, normal, medium, high] = curve;
            let curve = ContrastCurve::new(low, normal, medium, high);
            palettes
                .neutral_variant
                .tone(conditions.foreground(initial, curve))
        };
        Scheme {
            primary: palettes.primary.tone(conditions.primary()),
            error: ERROR_PALETTE.tone(conditions.accent(
                conditions.by_mode(30.0, 90.0),
                conditions.by_mode(80.0, 40.0),
            )),
            surface: neutral(conditions.by_mode(6.0, 98.0)),
            surface_container_lowest: neutral(match dark {
                true => ContrastCurve::new(4.0, 4.0, 2.0, 0.0).at(contrast),
                false => 100.0,
            }),
            surface_container_low: neutral(
                conditions.curved([10.0, 10.0, 11.0, 12.0], [96.0, 96.0, 96.0, 95.0]),
            ),
            surface_container: neutral(
                conditions.curved([12.0, 12.0, 16.0, 20.0], [94.0, 94.0, 92.0, 90.0]),
            ),
            surface_container_high: neutral(
                conditions.curved([17.0, 17.0, 21.0, 25.0], [92.0, 92.0, 88.0, 85.0]),
            ),
            surface_container_highest: neutral(
                conditions.curved([22.0, 22.0, 26.0, 30.0], [90.0, 90.0, 84.0, 80.0]),
            ),
            on_surface: neutral(conditions.foreground(
                conditions.by_mode(90.0, 10.0),
                ContrastCurve::new(4.5, 7.0, 11.0, 21.0),
            )),
            on_surface_variant: neutral_variant(
                conditions.by_mode(80.0, 30.0),
                [3.0, 4.5, 7.0, 11.0],
            ),
            outline: neutral_variant(conditions.by_mode(60.0, 50.0), [1.5, 3.0, 4.5, 7.0]),
            outline_variant: neutral_variant(conditions.by_mode(30.0, 80.0), [1.0, 1.0, 3.0, 4.5]),
        }
    }
}
