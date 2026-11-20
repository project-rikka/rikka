use std::fmt;

use rikka_types::Color;

const PERCENTILES: [f64; 5] = [0.01, 0.05, 0.5, 0.95, 0.99];
const BINS: usize = 16;
const CLIPPED_LOW: u8 = 1;
const CLIPPED_HIGH: u8 = 254;
const MIDTONES: std::ops::RangeInclusive<f64> = 25.0..=230.0;
const NEUTRAL: f64 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Statistics {
    pub pixels: u64,
    pub transparent: u64,
    pub mean: [f64; 3],
    pub percentiles: [u8; 5],
    pub shadows_clipped: f64,
    pub highlights_clipped: f64,
    pub saturation: f64,
    pub warmth: f64,
    pub tint: f64,
    pub histogram: [f64; BINS],
}

impl Statistics {
    pub fn measure(
        rgba: &[u8],
        width: u32,
        inside: impl Fn(u32, u32) -> bool,
    ) -> Option<Statistics> {
        let width = width.max(1);
        let mut sums = [0f64; 3];
        let mut luma = [0u64; 256];
        let (mut pixels, mut transparent, mut low, mut high) = (0u64, 0u64, 0u64, 0u64);
        let mut saturation = 0f64;
        let (mut middle, mut middle_sums) = (0u64, [0f64; 3]);
        for (at, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
            let (x, y) = (at as u32 % width, at as u32 / width);
            if !inside(x, y) {
                continue;
            }
            if pixel[3] == 0 {
                transparent += 1;
                continue;
            }
            pixels += 1;
            let [r, g, b] = [pixel[0], pixel[1], pixel[2]];
            for (sum, value) in sums.iter_mut().zip([r, g, b]) {
                *sum += f64::from(value);
            }
            let y = luma_of(r, g, b);
            luma[y.round() as usize] += 1;
            if r.min(g).min(b) <= CLIPPED_LOW {
                low += 1;
            }
            if r.max(g).max(b) >= CLIPPED_HIGH {
                high += 1;
            }
            let (most, least) = (r.max(g).max(b), r.min(g).min(b));
            if most > 0 {
                saturation += f64::from(most - least) / f64::from(most);
            }
            if MIDTONES.contains(&y) {
                middle += 1;
                for (sum, value) in middle_sums.iter_mut().zip([r, g, b]) {
                    *sum += f64::from(value);
                }
            }
        }
        if pixels == 0 {
            return None;
        }
        let count = pixels as f64;
        let mean = sums.map(|sum| sum / count);
        let cast = match middle {
            0 => mean,
            _ => middle_sums.map(|sum| sum / middle as f64),
        };
        let mut histogram = [0f64; BINS];
        for (level, counted) in luma.iter().enumerate() {
            histogram[level * BINS / 256] += *counted as f64 * 100.0 / count;
        }
        Some(Statistics {
            pixels,
            transparent,
            mean,
            percentiles: PERCENTILES.map(|fraction| percentile(&luma, pixels, fraction)),
            shadows_clipped: low as f64 * 100.0 / count,
            highlights_clipped: high as f64 * 100.0 / count,
            saturation: saturation * 100.0 / count,
            warmth: cast[0] - cast[2],
            tint: cast[1] - (cast[0] + cast[2]) / 2.0,
            histogram,
        })
    }
}

impl fmt::Display for Statistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b] = self.mean;
        let [p1, p5, p50, p95, p99] = self.percentiles;
        write!(
            f,
            "pixels {}{}\nmean rgb {:.0} {:.0} {:.0} (luma {:.0})\nluma percentiles 1/5/50/95/99: {p1} {p5} {p50} {p95} {p99}\nclipped: shadows {:.1}%, highlights {:.1}%\nmean saturation {:.0}%\ncast: {}\nluma histogram, 16 bins dark→light, %:",
            self.pixels,
            match self.transparent {
                0 => String::new(),
                transparent => format!(" (+{transparent} fully transparent, left out)"),
            },
            r,
            g,
            b,
            luma_of_mean(self.mean),
            self.shadows_clipped,
            self.highlights_clipped,
            self.saturation,
            cast(self.warmth, self.tint),
        )?;
        for share in self.histogram {
            write!(f, " {share:.0}")?;
        }
        Ok(())
    }
}

fn luma_of(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)
}

fn luma_of_mean([r, g, b]: [f64; 3]) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn percentile(luma: &[u64; 256], pixels: u64, fraction: f64) -> u8 {
    let target_count = ((pixels as f64 * fraction).ceil() as u64).max(1);
    let mut counted_total = 0;
    for (level, counted) in luma.iter().enumerate() {
        counted_total += counted;
        if counted_total >= target_count {
            return level as u8;
        }
    }
    u8::MAX
}

fn cast(warmth: f64, tint: f64) -> String {
    let lean = |value: f64, positive: &str, negative: &str| match value {
        value if value >= NEUTRAL => format!("{positive} by {value:.0}"),
        value if value <= -NEUTRAL => format!("{negative} by {:.0}", -value),
        _ => "neutral".to_owned(),
    };
    format!(
        "{} (midtone R−B {warmth:+.0}), {} (G − (R+B)/2 {tint:+.0})",
        lean(warmth, "warm", "cool"),
        lean(tint, "green", "magenta"),
    )
}

#[must_use]
pub fn sample_line(x: u32, y: u32, color: Color) -> String {
    format!("({x}, {y}) {}", color.to_hex())
}
