use crate::error::Result;
use crate::histogram::{BINS, Channel, Histogram, percentile_of, tone_means};
use crate::ops::Effect;
use crate::ops::tone::{Level, mapped};
use crate::picture::Frame;
use crate::work::Work;

const DEFAULT_CLIP: f32 = 0.001;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoLevels {
    clip: f32,
}

impl Default for AutoLevels {
    fn default() -> AutoLevels {
        AutoLevels { clip: DEFAULT_CLIP }
    }
}

impl AutoLevels {
    #[must_use]
    pub fn new() -> AutoLevels {
        AutoLevels::default()
    }

    #[must_use]
    pub fn clip(mut self, fraction: f32) -> AutoLevels {
        self.clip = fraction.clamp(0.0, 0.1);
        self
    }
}

impl Effect for AutoLevels {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let histogram = Histogram::from_plane(work, &frame.plane)?;
        let level = |channel| {
            Level::new().input(
                histogram.percentile(channel, self.clip),
                histogram.percentile(channel, 1.0 - self.clip),
            )
        };
        let [red, green, blue] = [Channel::Red, Channel::Green, Channel::Blue].map(level);
        let curve = |level: Level| move |value: f32| level.map(value);
        let (red, green, blue) = (curve(red), curve(green), curve(blue));
        mapped(work, frame, [None, Some(&red), Some(&green), Some(&blue)])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoContrast {
    clip: f32,
}

impl Default for AutoContrast {
    fn default() -> AutoContrast {
        AutoContrast { clip: DEFAULT_CLIP }
    }
}

impl AutoContrast {
    #[must_use]
    pub fn new() -> AutoContrast {
        AutoContrast::default()
    }

    #[must_use]
    pub fn clip(mut self, fraction: f32) -> AutoContrast {
        self.clip = fraction.clamp(0.0, 0.1);
        self
    }
}

fn combined_percentile(histogram: &Histogram, fraction: f32) -> f32 {
    let channels =
        [Channel::Red, Channel::Green, Channel::Blue].map(|channel| histogram.channel(channel));
    let bins = (0..BINS).map(|bin| {
        channels
            .iter()
            .map(|counts| u64::from(counts[bin]))
            .sum::<u64>()
    });
    percentile_of(bins, fraction)
}

impl Effect for AutoContrast {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let histogram = Histogram::from_plane(work, &frame.plane)?;
        let level = Level::new().input(
            combined_percentile(&histogram, self.clip),
            combined_percentile(&histogram, 1.0 - self.clip),
        );
        let curve = move |value: f32| level.map(value);
        mapped(work, frame, [Some(&curve), None, None, None])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoColor {
    clip: f32,
    snap_neutral: bool,
}

impl Default for AutoColor {
    fn default() -> AutoColor {
        AutoColor {
            clip: DEFAULT_CLIP,
            snap_neutral: true,
        }
    }
}

impl AutoColor {
    #[must_use]
    pub fn new() -> AutoColor {
        AutoColor::default()
    }

    #[must_use]
    pub fn clip(mut self, fraction: f32) -> AutoColor {
        self.clip = fraction.clamp(0.0, 0.1);
        self
    }

    #[must_use]
    pub fn snap_neutral(mut self, snap: bool) -> AutoColor {
        self.snap_neutral = snap;
        self
    }
}

impl Effect for AutoColor {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let histogram = Histogram::from_plane(work, &frame.plane)?;
        let margin = 1.0 / BINS as f32;
        let dark = histogram.percentile(Channel::Luminosity, self.clip.max(1e-4)) + margin;
        let light = histogram.percentile(Channel::Luminosity, 1.0 - self.clip.max(1e-4)) - margin;
        let means = tone_means(work, &frame.plane, dark, light)?;
        let black = means.dark.unwrap_or([0.0; 3]);
        let white = means.light.unwrap_or([1.0; 3]);
        let mut levels = [0, 1, 2].map(|channel| {
            let (low, high) = (black[channel], white[channel]);
            match high - low > 1e-3 {
                true => Level::new().input(low, high),
                false => Level::new(),
            }
        });
        if self.snap_neutral
            && let Some(neutral) = means.neutral
        {
            let placed = [0, 1, 2].map(|channel| {
                levels[channel]
                    .map(neutral[channel])
                    .clamp(1e-3, 1.0 - 1e-3)
            });
            let target = (placed[0] + placed[1] + placed[2]) / 3.0;
            for channel in 0..3 {
                let normalized = levels[channel]
                    .normalized(neutral[channel])
                    .clamp(1e-3, 1.0 - 1e-3);
                let gamma = (normalized.ln() / target.ln()).clamp(0.1, 9.99);
                levels[channel] = levels[channel].gamma(gamma);
            }
        }
        let [red, green, blue] = levels.map(|level| move |value: f32| level.map(value));
        mapped(work, frame, [None, Some(&red), Some(&green), Some(&blue)])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Equalize;

impl Effect for Equalize {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let histogram = Histogram::from_plane(work, &frame.plane)?;
        let bins = histogram.channel(Channel::Luminosity);
        let total = histogram.total().max(1) as f64;
        let mut running = 0u64;
        let cumulative: Vec<f32> = bins
            .iter()
            .map(|&count| {
                running += u64::from(count);
                (running as f64 / total) as f32
            })
            .collect();
        let curve = move |value: f32| {
            let position = value.clamp(0.0, 1.0) * (BINS - 1) as f32;
            let low = (position as usize).min(BINS - 2);
            let t = position - low as f32;
            cumulative[low] + (cumulative[low + 1] - cumulative[low]) * t
        };
        mapped(work, frame, [Some(&curve), None, None, None])
    }
}
