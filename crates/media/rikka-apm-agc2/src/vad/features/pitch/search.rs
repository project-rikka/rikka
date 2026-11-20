use crate::vad::common::{
    BUF_12K, BUF_24K, FRAME_20MS_12K, FRAME_20MS_24K, INITIAL_NUM_LAGS_24K, MAX_PITCH_24K,
    MIN_PITCH_24K, MIN_PITCH_48K, NUM_LAGS_12K, REFINE_NUM_LAGS_24K, dot,
};

const SUB_HARMONIC_MULTIPLIERS: [usize; 14] = [3, 2, 3, 2, 5, 2, 3, 2, 3, 2, 5, 2, 3, 2];
const INITIAL_PITCH_PERIOD_THRESHOLDS: [usize; 14] = [
    20, 45, 80, 125, 180, 245, 320, 405, 500, 605, 720, 845, 980, 1125,
];
const NEIGHBORHOOD_RADIUS: usize = 2;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PitchInfo {
    pub(crate) period: usize,
    pub(crate) strength: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Candidates {
    pub(crate) best: usize,
    pub(crate) second_best: usize,
}

#[derive(Clone, Copy, Debug)]
struct Range {
    min: usize,
    max: usize,
}

fn auto_correlation(inverted_lag: usize, pitch_buffer: &[f32; BUF_24K]) -> f32 {
    dot(
        &pitch_buffer[MAX_PITCH_24K..],
        &pitch_buffer[inverted_lag..inverted_lag + FRAME_20MS_24K],
    )
}

fn pseudo_interpolation_offset(prev: f32, curr: f32, next: f32) -> isize {
    if (next - prev) > 0.7 * (curr - prev) {
        1
    } else if (prev - next) > 0.7 * (curr - next) {
        -1
    } else {
        0
    }
}

fn pseudo_interpolate_lag(lag: usize, pitch_buffer: &[f32; BUF_24K]) -> usize {
    let mut offset = 0;
    if lag > 0 && lag < MAX_PITCH_24K {
        let inverted_lag = MAX_PITCH_24K - lag;
        offset = pseudo_interpolation_offset(
            auto_correlation(inverted_lag + 1, pitch_buffer),
            auto_correlation(inverted_lag, pitch_buffer),
            auto_correlation(inverted_lag - 1, pitch_buffer),
        );
    }
    (2 * lag).wrapping_add_signed(offset)
}

fn inverted_lag_range(inverted_lag: usize) -> Range {
    Range {
        min: inverted_lag.saturating_sub(NEIGHBORHOOD_RADIUS),
        max: (inverted_lag + NEIGHBORHOOD_RADIUS).min(INITIAL_NUM_LAGS_24K - 1),
    }
}

pub(crate) fn decimate_2x(source: &[f32; BUF_24K], target: &mut [f32; BUF_12K]) {
    for (index, value) in target.iter_mut().enumerate() {
        *value = source[2 * index];
    }
}

pub(crate) fn sliding_frame_energies_24k(
    pitch_buffer: &[f32; BUF_24K],
    energies: &mut [f32; REFINE_NUM_LAGS_24K],
) {
    let frame = &pitch_buffer[..FRAME_20MS_24K];
    let mut yy = dot(frame, frame);
    energies[0] = yy;
    for inverted_lag in 0..MAX_PITCH_24K {
        yy -= pitch_buffer[inverted_lag] * pitch_buffer[inverted_lag];
        yy += pitch_buffer[inverted_lag + FRAME_20MS_24K]
            * pitch_buffer[inverted_lag + FRAME_20MS_24K];
        yy = yy.max(1.0);
        energies[inverted_lag + 1] = yy;
    }
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    inverted_lag: usize,
    numerator: f32,
    denominator: f32,
}

impl Candidate {
    fn stronger_than(&self, other: &Candidate) -> bool {
        self.numerator * other.denominator > other.numerator * self.denominator
    }
}

pub(crate) fn pitch_period_12k(
    pitch_buffer: &[f32; BUF_12K],
    auto_corr: &[f32; NUM_LAGS_12K],
) -> Candidates {
    let frame = &pitch_buffer[..=FRAME_20MS_12K];
    let mut denominator = 1.0 + dot(frame, frame);
    let mut best = Candidate {
        inverted_lag: 0,
        numerator: -1.0,
        denominator: 0.0,
    };
    let mut second_best = Candidate {
        inverted_lag: 1,
        ..best
    };
    for (inverted_lag, &value) in auto_corr.iter().enumerate() {
        if value > 0.0 {
            let candidate = Candidate {
                inverted_lag,
                numerator: value * value,
                denominator,
            };
            if candidate.stronger_than(&second_best) {
                if candidate.stronger_than(&best) {
                    second_best = best;
                    best = candidate;
                } else {
                    second_best = candidate;
                }
            }
        }
        let old = pitch_buffer[inverted_lag];
        let new = pitch_buffer[inverted_lag + FRAME_20MS_12K];
        denominator -= old * old;
        denominator += new * new;
        denominator = denominator.max(0.0);
    }
    Candidates {
        best: best.inverted_lag,
        second_best: second_best.inverted_lag,
    }
}

pub(crate) fn pitch_period_48k(
    pitch_buffer: &[f32; BUF_24K],
    energies: &[f32; REFINE_NUM_LAGS_24K],
    candidates: Candidates,
) -> usize {
    let mut auto_corr = [0.0f32; INITIAL_NUM_LAGS_24K];
    let (low, high) = if candidates.best > candidates.second_best {
        (candidates.second_best, candidates.best)
    } else {
        (candidates.best, candidates.second_best)
    };
    let r1 = inverted_lag_range(low);
    let r2 = inverted_lag_range(high);
    let merged = [Range {
        min: r1.min,
        max: r2.max,
    }];
    let separate = [r1, r2];
    let ranges: &[Range] = if r1.max + 1 >= r2.min {
        &merged
    } else {
        &separate
    };
    for range in ranges {
        if range.min > 0 {
            auto_corr[range.min - 1] = 0.0;
        }
        if range.max < INITIAL_NUM_LAGS_24K - 1 {
            auto_corr[range.max + 1] = 0.0;
        }
        for (inverted_lag, value) in auto_corr
            .iter_mut()
            .enumerate()
            .take(range.max + 1)
            .skip(range.min)
        {
            *value = auto_correlation(inverted_lag, pitch_buffer);
        }
    }

    let mut best_inverted_lag = 0;
    let mut best_numerator = -1.0f32;
    let mut best_denominator = 0.0f32;
    for inverted_lag in ranges.iter().flat_map(|range| range.min..=range.max) {
        if auto_corr[inverted_lag] > 0.0 {
            let numerator = auto_corr[inverted_lag] * auto_corr[inverted_lag];
            let denominator = energies[inverted_lag];
            if numerator * best_denominator > best_numerator * denominator {
                best_inverted_lag = inverted_lag;
                best_numerator = numerator;
                best_denominator = denominator;
            }
        }
    }
    if best_inverted_lag == 0 || best_inverted_lag >= INITIAL_NUM_LAGS_24K - 1 {
        return best_inverted_lag * 2;
    }
    let offset = pseudo_interpolation_offset(
        auto_corr[best_inverted_lag + 1],
        auto_corr[best_inverted_lag],
        auto_corr[best_inverted_lag - 1],
    );
    (2 * best_inverted_lag).wrapping_add_signed(offset)
}

fn alternative_period(period: usize, multiplier: usize, divisor: usize) -> usize {
    (2 * multiplier * period + divisor) / (2 * divisor)
}

fn alternative_is_stronger(
    last: PitchInfo,
    initial: PitchInfo,
    alternative: PitchInfo,
    divisor: usize,
) -> bool {
    let mut lower_threshold_term = 0.0f32;
    let distance = alternative.period.abs_diff(last.period);
    if distance <= 1 {
        lower_threshold_term = last.strength;
    } else if distance == 2 && initial.period > INITIAL_PITCH_PERIOD_THRESHOLDS[divisor - 2] {
        lower_threshold_term = 0.5 * last.strength;
    }
    let mut threshold = 0.3f32.max(0.7 * initial.strength - lower_threshold_term);
    if alternative.period < 3 * MIN_PITCH_24K {
        threshold = 0.4f32.max(0.85 * initial.strength - lower_threshold_term);
    } else if alternative.period < 2 * MIN_PITCH_24K {
        threshold = 0.5f32.max(0.9 * initial.strength - lower_threshold_term);
    }
    alternative.strength > threshold
}

#[derive(Clone, Copy, Debug)]
struct Refined {
    period: usize,
    strength: f32,
    xy: f32,
    y_energy: f32,
}

pub(crate) fn extended_pitch_period_48k(
    pitch_buffer: &[f32; BUF_24K],
    energies: &[f32; REFINE_NUM_LAGS_24K],
    initial_period_48k: usize,
    last_48k: PitchInfo,
) -> PitchInfo {
    let x_energy = energies[MAX_PITCH_24K];
    let pitch_strength = |xy: f32, y_energy: f32| xy / (1.0 + x_energy * y_energy).sqrt();

    let period = (initial_period_48k / 2).min(MAX_PITCH_24K - 1);
    let xy = auto_correlation(MAX_PITCH_24K - period, pitch_buffer);
    let y_energy = energies[MAX_PITCH_24K - period];
    let mut best = Refined {
        period,
        strength: pitch_strength(xy, y_energy),
        xy,
        y_energy,
    };
    let initial = PitchInfo {
        period: best.period,
        strength: best.strength,
    };
    let last = PitchInfo {
        period: last_48k.period / 2,
        strength: last_48k.strength,
    };

    let max_divisor = (2 * initial.period) / (2 * MIN_PITCH_24K - 1);
    for divisor in 2..=max_divisor {
        let primary_period = alternative_period(initial.period, 1, divisor);
        let mut dual_period = alternative_period(
            initial.period,
            SUB_HARMONIC_MULTIPLIERS[divisor - 2],
            divisor,
        );
        if divisor == 2 && dual_period > MAX_PITCH_24K {
            dual_period = initial.period;
        }
        let xy_primary = auto_correlation(MAX_PITCH_24K - primary_period, pitch_buffer);
        let xy_secondary = auto_correlation(MAX_PITCH_24K - dual_period, pitch_buffer);
        let xy = 0.5 * (xy_primary + xy_secondary);
        let yy = 0.5
            * (energies[MAX_PITCH_24K - primary_period] + energies[MAX_PITCH_24K - dual_period]);
        let alternative = PitchInfo {
            period: primary_period,
            strength: pitch_strength(xy, yy),
        };
        if alternative_is_stronger(last, initial, alternative, divisor) {
            best = Refined {
                period: alternative.period,
                strength: alternative.strength,
                xy,
                y_energy: yy,
            };
        }
    }

    best.xy = best.xy.max(0.0);
    let mut final_strength = if best.y_energy <= best.xy {
        1.0
    } else {
        best.xy / (best.y_energy + 1.0)
    };
    final_strength = best.strength.min(final_strength);
    let final_period = MIN_PITCH_48K.max(pseudo_interpolate_lag(best.period, pitch_buffer));
    PitchInfo {
        period: final_period,
        strength: final_strength,
    }
}
