use std::cmp::Ordering;

use crate::color::Rgb;
use crate::hct::Hct;
use crate::math::{difference_degrees, sanitize_degrees_int};

const DESIRED: usize = 4;
const FALLBACK: Rgb = Rgb::new(66, 133, 244);
const TARGET_CHROMA: f64 = 48.0;
const WEIGHT_PROPORTION: f64 = 0.7;
const WEIGHT_CHROMA_ABOVE: f64 = 0.3;
const WEIGHT_CHROMA_BELOW: f64 = 0.1;
const CUTOFF_CHROMA: f64 = 5.0;
const CUTOFF_EXCITED_PROPORTION: f64 = 0.01;
const HUES: usize = 360;

#[must_use]
pub fn score(colors: &[(Rgb, u32)]) -> Vec<Rgb> {
    let mut hue_population = [0u32; HUES];
    let mut population_sum = 0.0;
    let measured: Vec<Hct> = colors
        .iter()
        .map(|&(color, population)| {
            let hct = Hct::of(color);
            hue_population[(hct.hue().floor() as usize).min(HUES - 1)] += population;
            population_sum += f64::from(population);
            hct
        })
        .collect();
    let mut excited = [0.0f64; HUES];
    for (hue, population) in hue_population.into_iter().enumerate() {
        let proportion = f64::from(population) / population_sum;
        for neighbour in (hue as i32 - 14)..(hue as i32 + 16) {
            excited[sanitize_degrees_int(neighbour)] += proportion;
        }
    }
    let mut scored: Vec<(Hct, f64)> = Vec::new();
    for hct in measured {
        let proportion = excited[sanitize_degrees_int(hct.hue().round() as i32)];
        if hct.chroma() < CUTOFF_CHROMA || proportion <= CUTOFF_EXCITED_PROPORTION {
            continue;
        }
        let chroma_weight = if hct.chroma() < TARGET_CHROMA {
            WEIGHT_CHROMA_BELOW
        } else {
            WEIGHT_CHROMA_ABOVE
        };
        let score =
            proportion * 100.0 * WEIGHT_PROPORTION + (hct.chroma() - TARGET_CHROMA) * chroma_weight;
        scored.push((hct, score));
    }
    scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal));
    let mut chosen: Vec<Hct> = Vec::new();
    for apart in (15..=90).rev() {
        chosen.clear();
        for &(hct, _) in &scored {
            let crowded = chosen
                .iter()
                .any(|kept| difference_degrees(hct.hue(), kept.hue()) < f64::from(apart));
            if !crowded {
                chosen.push(hct);
            }
            if chosen.len() >= DESIRED {
                break;
            }
        }
        if chosen.len() >= DESIRED {
            break;
        }
    }
    if chosen.is_empty() {
        return vec![FALLBACK];
    }
    chosen.into_iter().map(Hct::rgb).collect()
}
