use glam::{Quat, Vec3};

use crate::animation::{Channel, Curve, Track};
use crate::node::NodeId;
use crate::transform::Transform;

const DISTANCE: f32 = 1e-4;
const ANGLE: f32 = 1e-4;
const WEIGHT: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransformParts {
    pub translation: bool,
    pub rotation: bool,
    pub scale: bool,
}

impl TransformParts {
    pub const ALL: Self = Self {
        translation: true,
        rotation: true,
        scale: true,
    };

    pub const PLACEMENT: Self = Self {
        translation: true,
        rotation: true,
        scale: false,
    };
}

#[derive(Clone, Debug, Default)]
pub struct TransformSamples {
    times: Vec<f32>,
    translations: Vec<Vec3>,
    rotations: Vec<Quat>,
    scales: Vec<Vec3>,
}

impl TransformSamples {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            times: Vec::with_capacity(capacity),
            translations: Vec::with_capacity(capacity),
            rotations: Vec::with_capacity(capacity),
            scales: Vec::with_capacity(capacity),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.times.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    #[must_use]
    pub fn last_rotation(&self) -> Option<Quat> {
        self.rotations.last().copied()
    }

    pub fn push(&mut self, time: f32, transform: Transform) {
        self.times.push(time);
        self.translations.push(transform.translation);
        self.rotations.push(transform.rotation);
        self.scales.push(transform.scale);
    }

    #[must_use]
    pub fn channels(self, target: NodeId, parts: TransformParts) -> Vec<Channel> {
        let Self {
            times,
            translations,
            rotations,
            scales,
        } = self;
        let mut channels = Vec::with_capacity(3);
        if parts.translation {
            channels.push(Channel {
                target,
                track: Track::Translation(Curve::linear(times.clone(), translations)),
            });
        }
        if parts.rotation {
            channels.push(Channel {
                target,
                track: Track::Rotation(Curve::linear(times.clone(), rotations)),
            });
        }
        if parts.scale {
            channels.push(Channel {
                target,
                track: Track::Scale(Curve::linear(times, scales)),
            });
        }
        channels
    }
}

pub trait Sample: Clone {
    fn strays(&self, start: &Self, end: &Self, factor: f32) -> bool;
}

impl Sample for Transform {
    fn strays(&self, start: &Self, end: &Self, factor: f32) -> bool {
        let straight = start.lerp(*end, factor);
        let apart = |a: Vec3, b: Vec3| (a - b).length() > DISTANCE * (1.0 + b.length());
        apart(straight.translation, self.translation)
            || apart(straight.scale, self.scale)
            || 2.0 * (straight.rotation.inverse() * self.rotation).xyz().length() > ANGLE
    }
}

impl Sample for Vec<f32> {
    fn strays(&self, start: &Self, end: &Self, factor: f32) -> bool {
        self.iter()
            .zip(start.iter().zip(end))
            .any(|(actual, (from, to))| (from + (to - from) * factor - actual).abs() > WEIGHT)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Subdivision {
    Halves { rounds: u32 },
    Grid { step: f64 },
}

impl Subdivision {
    fn split(self, from: f64, to: f64, depth: u32) -> Option<f64> {
        let middle = (from + to) * 0.5;
        let inside = |at: f64| at > from && at < to;
        match self {
            Self::Halves { rounds } => (depth < rounds && inside(middle)).then_some(middle),
            Self::Grid { step } => {
                let low = (middle / step).floor() * step;
                let high = (middle / step).ceil() * step;
                let near = match middle - low <= high - middle {
                    true => [low, high],
                    false => [high, low],
                };
                near.into_iter().find(|&at| inside(at))
            }
        }
    }
}

pub fn adaptive<V: Sample>(
    mut seeds: Vec<f64>,
    subdivision: Subdivision,
    mut sample: impl FnMut(f64) -> V,
) -> Vec<(f64, V)> {
    seeds.retain(|seed| seed.is_finite());
    seeds.sort_by(f64::total_cmp);
    seeds.dedup();
    let mut seeds = seeds.into_iter();
    let Some(first) = seeds.next() else {
        return Vec::new();
    };
    let mut bisection = Bisection {
        subdivision,
        sample: &mut sample,
        baked: Vec::with_capacity(seeds.len() + 1),
    };
    let mut start = (first, (bisection.sample)(first));
    for at in seeds {
        let end = (at, (bisection.sample)(at));
        bisection.baked.push(start.clone());
        bisection.segment(&start, &end, None, 0);
        start = end;
    }
    bisection.baked.push(start);
    bisection.baked
}

struct Bisection<'s, V, F> {
    subdivision: Subdivision,
    sample: &'s mut F,
    baked: Vec<(f64, V)>,
}

impl<V: Sample, F: FnMut(f64) -> V> Bisection<'_, V, F> {
    fn probe(&mut self, from: f64, to: f64, depth: u32) -> Option<(f64, V)> {
        let at = self.subdivision.split(from, to, depth)?;
        Some((at, (self.sample)(at)))
    }

    fn segment(&mut self, start: &(f64, V), end: &(f64, V), middle: Option<(f64, V)>, depth: u32) {
        let Some(middle) = middle.or_else(|| self.probe(start.0, end.0, depth)) else {
            return;
        };
        let left = self.probe(start.0, middle.0, depth + 1);
        let right = self.probe(middle.0, end.0, depth + 1);
        let strays = |(at, value): &(f64, V)| {
            value.strays(
                &start.1,
                &end.1,
                ((at - start.0) / (end.0 - start.0)) as f32,
            )
        };
        if !(strays(&middle)
            || left.as_ref().is_some_and(strays)
            || right.as_ref().is_some_and(strays))
        {
            return;
        }
        self.segment(start, &middle, left, depth + 1);
        self.baked.push(middle.clone());
        self.segment(&middle, end, right, depth + 1);
    }
}
