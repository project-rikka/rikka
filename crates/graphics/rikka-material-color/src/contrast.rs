use crate::color::{lstar_from_y, y_from_lstar};
use crate::math::lerp;

const TONES: std::ops::RangeInclusive<f64> = 0.0..=100.0;
const UNREACHABLE_MARGIN: f64 = 0.04;
const TONE_SLACK: f64 = 0.4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ContrastCurve {
    pub(crate) low: f64,
    pub(crate) normal: f64,
    pub(crate) medium: f64,
    pub(crate) high: f64,
}

impl ContrastCurve {
    pub(crate) const fn new(low: f64, normal: f64, medium: f64, high: f64) -> Self {
        Self {
            low,
            normal,
            medium,
            high,
        }
    }

    pub(crate) fn at(&self, level: f64) -> f64 {
        if level <= -1.0 {
            self.low
        } else if level < 0.0 {
            lerp(self.low, self.normal, (level - (-1.0)) / 1.0)
        } else if level < 0.5 {
            lerp(self.normal, self.medium, (level - 0.0) / 0.5)
        } else if level < 1.0 {
            lerp(self.medium, self.high, (level - 0.5) / 0.5)
        } else {
            self.high
        }
    }
}

pub(crate) fn ratio_of_tones(tone_a: f64, tone_b: f64) -> f64 {
    ratio_of_ys(
        y_from_lstar(tone_a.clamp(0.0, 100.0)),
        y_from_lstar(tone_b.clamp(0.0, 100.0)),
    )
}

fn ratio_of_ys(y1: f64, y2: f64) -> f64 {
    let lighter = if y1 > y2 { y1 } else { y2 };
    let darker = if (lighter - y2).abs() < f64::EPSILON {
        y1
    } else {
        y2
    };
    (lighter + 5.0) / (darker + 5.0)
}

fn lighter(tone: f64, ratio: f64) -> Option<f64> {
    if !TONES.contains(&tone) {
        return None;
    }
    let dark_y = y_from_lstar(tone);
    let light_y = ratio.mul_add(dark_y + 5.0, -5.0);
    let real_contrast = ratio_of_ys(light_y, dark_y);
    if real_contrast < ratio && (real_contrast - ratio).abs() > UNREACHABLE_MARGIN {
        return None;
    }
    Some(lstar_from_y(light_y) + TONE_SLACK).filter(|tone| TONES.contains(tone))
}

fn darker(tone: f64, ratio: f64) -> Option<f64> {
    if !TONES.contains(&tone) {
        return None;
    }
    let light_y = y_from_lstar(tone);
    let dark_y = ((light_y + 5.0) / ratio) - 5.0;
    let real_contrast = ratio_of_ys(light_y, dark_y);
    if real_contrast < ratio && (real_contrast - ratio).abs() > UNREACHABLE_MARGIN {
        return None;
    }
    Some(lstar_from_y(dark_y) - TONE_SLACK).filter(|tone| TONES.contains(tone))
}

pub(crate) fn prefers_light_foreground(tone: f64) -> bool {
    tone.round() < 60.0
}

pub(crate) fn foreground_tone(background_tone: f64, ratio: f64) -> f64 {
    let lighter_tone = lighter(background_tone, ratio).unwrap_or(100.0);
    let darker_tone = darker(background_tone, ratio).unwrap_or(0.0);
    let lighter_ratio = ratio_of_tones(lighter_tone, background_tone);
    let darker_ratio = ratio_of_tones(darker_tone, background_tone);
    if prefers_light_foreground(background_tone) {
        let negligible_difference = (lighter_ratio - darker_ratio).abs() < 0.1
            && lighter_ratio < ratio
            && darker_ratio < ratio;
        if lighter_ratio >= ratio || lighter_ratio >= darker_ratio || negligible_difference {
            lighter_tone
        } else {
            darker_tone
        }
    } else if darker_ratio >= ratio || darker_ratio >= lighter_ratio {
        darker_tone
    } else {
        lighter_tone
    }
}
