use crate::CHANNELS;
use crate::samples::energy;

const MIN_LAG: usize = 120;
const MAX_LAG: usize = 480;
const VOICED: f32 = 0.9;
const QUIET_ENERGY: f32 = 1.0e-5;
const QUIET_LAG: usize = 480;
const DECIMATION: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stretch {
    Shorten,
    Lengthen,
}

pub(crate) fn stretch(block: &mut Vec<f32>, how: Stretch) -> bool {
    let frames = block.len() / CHANNELS;
    let mono: Vec<f32> = block
        .as_chunks::<CHANNELS>()
        .0
        .iter()
        .map(|frame| frame.iter().sum::<f32>() / CHANNELS as f32)
        .collect();
    let Some(lag) = choose_lag(&mono, frames) else {
        return false;
    };
    let lag_samples = lag * CHANNELS;
    let first = block[..lag_samples].to_vec();
    let second = block[lag_samples..2 * lag_samples].to_vec();
    let rest = block[2 * lag_samples..].to_vec();
    block.clear();
    match how {
        Stretch::Shorten => {
            block.extend(crossfade(&first, &second, lag));
        }
        Stretch::Lengthen => {
            block.extend_from_slice(&first);
            block.extend(crossfade(&second, &first, lag));
            block.extend_from_slice(&second);
        }
    }
    block.extend_from_slice(&rest);
    true
}

fn choose_lag(mono: &[f32], frames: usize) -> Option<usize> {
    let longest = MAX_LAG.min(frames / 2);
    if longest < MIN_LAG {
        return None;
    }
    if energy(mono) < QUIET_ENERGY {
        return Some(QUIET_LAG.min(longest));
    }
    let coarse: Vec<f32> = mono
        .as_chunks::<DECIMATION>()
        .0
        .iter()
        .map(|chunk| chunk.iter().sum::<f32>())
        .collect();
    let mut best = (0.0_f32, 0_usize);
    for lag in (MIN_LAG / DECIMATION)..=(longest / DECIMATION) {
        let score = correlation(&coarse[..lag], &coarse[lag..2 * lag]);
        if score > best.0 {
            best = (score, lag * DECIMATION);
        }
    }
    let (_, coarse_lag) = best;
    if coarse_lag == 0 {
        return None;
    }
    let low = coarse_lag.saturating_sub(DECIMATION).max(MIN_LAG);
    let high = (coarse_lag + DECIMATION).min(longest);
    let mut refined = (f32::MIN, coarse_lag);
    for lag in low..=high {
        let score = correlation(&mono[..lag], &mono[lag..2 * lag]);
        if score > refined.0 {
            refined = (score, lag);
        }
    }
    (refined.0 >= VOICED).then_some(refined.1)
}

fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let (mut cross, mut left, mut right) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (x, y) in a.iter().zip(b) {
        cross += x * y;
        left += x * x;
        right += y * y;
    }
    let scale = (left * right).sqrt();
    if scale <= f32::EPSILON {
        return 0.0;
    }
    cross / scale
}

fn crossfade<'a>(from: &'a [f32], to: &'a [f32], frames: usize) -> impl Iterator<Item = f32> + 'a {
    from.as_chunks::<CHANNELS>()
        .0
        .iter()
        .zip(to.as_chunks::<CHANNELS>().0)
        .enumerate()
        .flat_map(move |(index, (a, b))| {
            let fade = (index as f32 + 0.5) / frames as f32;
            a.iter()
                .zip(b)
                .map(move |(x, y)| x * (1.0 - fade) + y * fade)
        })
}
