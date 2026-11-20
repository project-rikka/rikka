use crate::error::Result;
use crate::ops::passes::adjust;
use crate::ops::spline::Spline;
use crate::ops::{Effect, kind};
use crate::picture::Frame;
use crate::work::Work;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    Composite,
    Red,
    Green,
    Blue,
}

impl Channel {
    fn index(self) -> usize {
        self as usize
    }
}

struct Tables {
    size: usize,
    values: Vec<f32>,
}

impl Tables {
    fn identity(size: usize) -> Tables {
        let last = (size - 1) as f32;
        let ramp: Vec<f32> = (0..size).map(|index| index as f32 / last).collect();
        Tables {
            size,
            values: ramp.repeat(4),
        }
    }

    fn set(&mut self, channel: Channel, curve: impl Fn(f32) -> f32) {
        let last = (self.size - 1) as f32;
        let at = channel.index() * self.size;
        for index in 0..self.size {
            self.values[at + index] = curve(index as f32 / last).clamp(0.0, 1.0);
        }
    }

    fn apply(self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        adjust(work, frame, kind::CURVES, &self.values, self.size as u32)
    }
}

pub(crate) fn mapped(
    work: &mut Work,
    frame: &Frame,
    curves: [Option<&dyn Fn(f32) -> f32>; 4],
) -> Result<Frame> {
    let mut tables = Tables::identity(frame.depth.steps());
    for (channel, curve) in [
        Channel::Composite,
        Channel::Red,
        Channel::Green,
        Channel::Blue,
    ]
    .into_iter()
    .zip(curves)
    {
        if let Some(curve) = curve {
            tables.set(channel, curve);
        }
    }
    tables.apply(work, frame)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    input: (f32, f32),
    gamma: f32,
    output: (f32, f32),
}

impl Default for Level {
    fn default() -> Level {
        Level {
            input: (0.0, 1.0),
            gamma: 1.0,
            output: (0.0, 1.0),
        }
    }
}

impl Level {
    #[must_use]
    pub fn new() -> Level {
        Level::default()
    }

    #[must_use]
    pub fn input(mut self, black: f32, white: f32) -> Level {
        self.input = (black.clamp(0.0, 1.0), white.clamp(0.0, 1.0));
        self
    }

    #[must_use]
    pub fn gamma(mut self, gamma: f32) -> Level {
        self.gamma = gamma.clamp(0.01, 9.99);
        self
    }

    #[must_use]
    pub fn output(mut self, black: f32, white: f32) -> Level {
        self.output = (black.clamp(0.0, 1.0), white.clamp(0.0, 1.0));
        self
    }

    fn is_identity(&self) -> bool {
        *self == Level::default()
    }

    pub(crate) fn normalized(&self, value: f32) -> f32 {
        let (black, white) = self.input;
        let range = match (white - black).abs() > 1e-6 {
            true => white - black,
            false => 1.0,
        };
        ((value - black) / range).clamp(0.0, 1.0)
    }

    pub(crate) fn map(&self, value: f32) -> f32 {
        let shaped = self.normalized(value).powf(1.0 / self.gamma);
        self.output.0 + shaped * (self.output.1 - self.output.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Levels {
    channels: [Level; 4],
}

impl Levels {
    #[must_use]
    pub fn new() -> Levels {
        Levels::default()
    }

    #[must_use]
    pub fn channel(mut self, channel: Channel, level: Level) -> Levels {
        self.channels[channel.index()] = level;
        self
    }
}

impl Effect for Levels {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let mut tables = Tables::identity(frame.depth.steps());
        for channel in [
            Channel::Composite,
            Channel::Red,
            Channel::Green,
            Channel::Blue,
        ] {
            let level = self.channels[channel.index()];
            if !level.is_identity() {
                tables.set(channel, |value| level.map(value));
            }
        }
        tables.apply(work, frame)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Curves {
    channels: [Vec<(f32, f32)>; 4],
}

impl Curves {
    #[must_use]
    pub fn new() -> Curves {
        Curves::default()
    }

    #[must_use]
    pub fn channel(
        mut self,
        channel: Channel,
        points: impl IntoIterator<Item = (f32, f32)>,
    ) -> Curves {
        self.channels[channel.index()] = points
            .into_iter()
            .map(|(input, output)| (input.clamp(0.0, 1.0), output.clamp(0.0, 1.0)))
            .collect();
        self
    }
}

impl Effect for Curves {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let mut tables = Tables::identity(frame.depth.steps());
        for channel in [
            Channel::Composite,
            Channel::Red,
            Channel::Green,
            Channel::Blue,
        ] {
            let points = &self.channels[channel.index()];
            if points.len() < 2 {
                continue;
            }
            let spline = Spline::from_points(
                points
                    .iter()
                    .map(|&(input, output)| (f64::from(input), f64::from(output))),
            );
            tables.set(channel, |value| spline.evaluate(f64::from(value)) as f32);
        }
        tables.apply(work, frame)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BrightnessContrast {
    brightness: f32,
    contrast: f32,
    legacy: bool,
}

impl BrightnessContrast {
    #[must_use]
    pub fn new(brightness: f32, contrast: f32) -> BrightnessContrast {
        BrightnessContrast {
            brightness: brightness.clamp(-150.0, 150.0),
            contrast: contrast.clamp(-100.0, 100.0),
            legacy: false,
        }
    }

    #[must_use]
    pub fn legacy(mut self, legacy: bool) -> BrightnessContrast {
        self.legacy = legacy;
        self
    }

    fn contrast(&self) -> f32 {
        match self.legacy {
            true => self.contrast,
            false => self.contrast.max(-50.0),
        }
    }

    pub(crate) fn curve(&self, size: usize) -> Vec<f32> {
        let last = (size - 1) as f32;
        if self.legacy {
            let shift = self.brightness / 255.0;
            let slope = ((self.contrast() / 100.0 + 1.0) * std::f32::consts::FRAC_PI_4)
                .tan()
                .clamp(0.0, 255.0);
            return (0..size)
                .map(|index| ((index as f32 / last + shift - 0.5) * slope + 0.5).clamp(0.0, 1.0))
                .collect();
        }
        let b = f64::from(self.brightness / 150.0);
        let c = f64::from(self.contrast() / 100.0);
        let xs = [0.0, 63.0 / 255.0, 191.0 / 255.0, 1.0];
        let delta = 25.0 / 255.0;
        let ys = [0.0, xs[1] - c * delta, xs[2] + c * delta, 1.0];
        let contrast = Spline::new(&xs, &ys);
        let magnitude = b.abs();
        let term = |a: f64, x: f64, r: f64| a * x.powf(r);
        let samples: Vec<(f64, f64)> = (0..1024)
            .map(|index| {
                let t = f64::from(index) / 1023.0;
                let h = 0.5
                    * (magnitude * (term(1.65, t, 0.35) + term(-1.0, t, 10.0))
                        + (1.0 - magnitude) * (term(1.96, t, 0.4) + term(1.0, t, 4.0))
                        + term(1.0, t, 1.25));
                let shift = b * t * (1.0 - t) * h;
                (t - shift, contrast.evaluate(t) + shift)
            })
            .collect();
        (0..size)
            .map(|index| {
                let value = f64::from(index as f32 / last);
                let upper = samples
                    .partition_point(|sample| sample.0 < value)
                    .clamp(1, samples.len() - 1);
                let (a, b) = (samples[upper - 1], samples[upper]);
                let span = b.0 - a.0;
                let fraction = match span.abs() < 1e-12 {
                    true => 0.0,
                    false => ((value - a.0) / span).clamp(0.0, 1.0),
                };
                ((a.1 + (b.1 - a.1) * fraction) as f32).clamp(0.0, 1.0)
            })
            .collect()
    }
}

impl Effect for BrightnessContrast {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let size = frame.depth.steps();
        let mut tables = Tables::identity(size);
        tables.values[..size].copy_from_slice(&self.curve(size));
        tables.apply(work, frame)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exposure {
    exposure: f32,
    offset: f32,
    gamma: f32,
}

impl Exposure {
    #[must_use]
    pub fn new(exposure: f32) -> Exposure {
        Exposure {
            exposure: exposure.clamp(-20.0, 20.0),
            offset: 0.0,
            gamma: 1.0,
        }
    }

    #[must_use]
    pub fn offset(mut self, offset: f32) -> Exposure {
        self.offset = offset.clamp(-0.5, 0.5);
        self
    }

    #[must_use]
    pub fn gamma(mut self, gamma: f32) -> Exposure {
        self.gamma = gamma.clamp(0.01, 9.99);
        self
    }
}

impl Effect for Exposure {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        adjust(
            work,
            frame,
            kind::EXPOSURE,
            &[self.exposure, self.offset, self.gamma],
            0,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Posterize {
    levels: u16,
}

impl Posterize {
    #[must_use]
    pub fn new(levels: u16) -> Posterize {
        Posterize {
            levels: levels.clamp(2, 255),
        }
    }
}

impl Effect for Posterize {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let levels = f32::from(self.levels);
        let mut tables = Tables::identity(frame.depth.steps());
        tables.set(Channel::Composite, |value| {
            (value * levels * 0.999_99).floor() / (levels - 1.0)
        });
        tables.apply(work, frame)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Invert;

impl Effect for Invert {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        adjust(work, frame, kind::INVERT, &[], 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Threshold {
    level: f32,
}

impl Threshold {
    #[must_use]
    pub fn new(level: f32) -> Threshold {
        Threshold {
            level: level.clamp(0.0, 1.0),
        }
    }
}

impl Default for Threshold {
    fn default() -> Threshold {
        Threshold::new(0.5)
    }
}

impl Effect for Threshold {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        adjust(work, frame, kind::THRESHOLD, &[self.level], 0)
    }
}
