use glam::{Quat, Vec3, Vec4};

use crate::error::{Error, Result};
use crate::id::model_id;
use crate::node::NodeId;
use crate::pose::Pose;

model_id!(AnimationId);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interpolation {
    Step,
    #[default]
    Linear,
    CubicSpline,
}

impl Interpolation {
    #[must_use]
    pub const fn stride(self) -> usize {
        match self {
            Self::CubicSpline => 3,
            _ => 1,
        }
    }
}

pub trait Interpolate: Copy {
    fn mix(self, other: Self, factor: f32) -> Self;
    fn scaled(self, factor: f32) -> Self;
    fn combined(self, other: Self) -> Self;
    fn normalized(self) -> Self;
}

impl Interpolate for f32 {
    fn mix(self, other: Self, factor: f32) -> Self {
        self + (other - self) * factor
    }

    fn scaled(self, factor: f32) -> Self {
        self * factor
    }

    fn combined(self, other: Self) -> Self {
        self + other
    }

    fn normalized(self) -> Self {
        self
    }
}

impl Interpolate for Vec3 {
    fn mix(self, other: Self, factor: f32) -> Self {
        self.lerp(other, factor)
    }

    fn scaled(self, factor: f32) -> Self {
        self * factor
    }

    fn combined(self, other: Self) -> Self {
        self + other
    }

    fn normalized(self) -> Self {
        self
    }
}

impl Interpolate for Quat {
    fn mix(self, other: Self, factor: f32) -> Self {
        self.slerp(other, factor)
    }

    fn scaled(self, factor: f32) -> Self {
        Quat::from_vec4(Vec4::from(self) * factor)
    }

    fn combined(self, other: Self) -> Self {
        Quat::from_vec4(Vec4::from(self) + Vec4::from(other))
    }

    fn normalized(self) -> Self {
        self.normalize()
    }
}

#[derive(Clone, Debug)]
pub struct Curve<T> {
    pub times: Vec<f32>,
    pub values: Vec<T>,
    pub interpolation: Interpolation,
}

impl<T> Default for Curve<T> {
    fn default() -> Self {
        Self {
            times: Vec::new(),
            values: Vec::new(),
            interpolation: Interpolation::default(),
        }
    }
}

impl<T: Interpolate> Curve<T> {
    pub fn linear(times: Vec<f32>, values: Vec<T>) -> Self {
        Self {
            times,
            values,
            interpolation: Interpolation::Linear,
        }
    }

    #[must_use]
    pub fn duration(&self) -> f32 {
        self.times.last().copied().unwrap_or(0.0)
    }

    #[must_use]
    pub fn key(&self, index: usize) -> Option<T> {
        let stride = self.interpolation.stride();
        self.values.get(index * stride + stride / 2).copied()
    }

    #[must_use]
    pub fn sample(&self, time: f32) -> Option<T> {
        let stride = self.interpolation.stride();
        let key = |index: usize| self.key(index);
        let Span { start, end, factor } = span(&self.times, time)?;
        if start == end {
            return key(start);
        }
        match self.interpolation {
            Interpolation::Step => key(start),
            Interpolation::Linear => Some(key(start)?.mix(key(end)?, factor)),
            Interpolation::CubicSpline => {
                let delta = self.times[end] - self.times[start];
                Some(hermite(
                    key(start)?,
                    self.values.get(start * stride + 2).copied()?.scaled(delta),
                    key(end)?,
                    self.values.get(end * stride).copied()?.scaled(delta),
                    factor,
                ))
            }
        }
    }

    pub(crate) fn validate(&self, what: &str) -> Result<()> {
        let expected = self.times.len() * self.interpolation.stride();
        if self.values.len() != expected {
            return Err(Error::Invalid(format!(
                "{what} has {} keyframes and {} values, expected {expected}",
                self.times.len(),
                self.values.len()
            )));
        }
        ascending(&self.times, what)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Weights {
    pub times: Vec<f32>,
    pub values: Vec<f32>,
    pub count: usize,
    pub interpolation: Interpolation,
}

impl Weights {
    #[must_use]
    pub fn duration(&self) -> f32 {
        self.times.last().copied().unwrap_or(0.0)
    }

    #[must_use]
    pub fn key(&self, index: usize) -> Option<&[f32]> {
        let stride = self.interpolation.stride();
        let first = (index * stride + stride / 2) * self.count;
        self.values.get(first..first + self.count)
    }

    pub fn sample_into(&self, time: f32, out: &mut [f32]) -> bool {
        let stride = self.interpolation.stride();
        let Some(Span { start, end, factor }) = span(&self.times, time) else {
            return false;
        };
        let block = |index: usize| {
            let first = index * self.count;
            self.values.get(first..first + self.count)
        };
        let key = |index: usize| self.key(index);
        let (Some(from), Some(to)) = (key(start), key(end)) else {
            return false;
        };
        let tangents = match self.interpolation {
            Interpolation::CubicSpline => match (block(start * stride + 2), block(end * stride)) {
                (Some(out_tangent), Some(in_tangent)) => Some((out_tangent, in_tangent)),
                _ => return false,
            },
            _ => None,
        };
        let width = self.times[end] - self.times[start];

        let taken = out.len().min(self.count);
        for slot in 0..taken {
            out[slot] = match (self.interpolation, tangents) {
                (Interpolation::Step, _) => from[slot],
                (Interpolation::CubicSpline, Some((out_tangent, in_tangent))) => hermite(
                    from[slot],
                    out_tangent[slot] * width,
                    to[slot],
                    in_tangent[slot] * width,
                    factor,
                ),
                _ => from[slot].mix(to[slot], factor),
            };
        }
        true
    }

    pub(crate) fn validate(&self, what: &str) -> Result<()> {
        let expected = self.times.len() * self.interpolation.stride() * self.count;
        if self.values.len() != expected {
            return Err(Error::Invalid(format!(
                "{what} has {} keyframes of {} weights and {} values, expected {expected}",
                self.times.len(),
                self.count,
                self.values.len()
            )));
        }
        ascending(&self.times, what)
    }
}

#[derive(Clone, Debug)]
pub enum Track {
    Translation(Curve<Vec3>),
    Rotation(Curve<Quat>),
    Scale(Curve<Vec3>),
    Weights(Weights),
}

impl Track {
    #[must_use]
    pub fn duration(&self) -> f32 {
        match self {
            Self::Translation(curve) | Self::Scale(curve) => curve.duration(),
            Self::Rotation(curve) => curve.duration(),
            Self::Weights(weights) => weights.duration(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Translation(curve) | Self::Scale(curve) => curve.times.len(),
            Self::Rotation(curve) => curve.times.len(),
            Self::Weights(weights) => weights.times.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn validate(&self, what: &str) -> Result<()> {
        match self {
            Self::Translation(curve) | Self::Scale(curve) => curve.validate(what),
            Self::Rotation(curve) => curve.validate(what),
            Self::Weights(weights) => weights.validate(what),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub target: NodeId,
    pub track: Track,
}

#[derive(Clone, Debug, Default)]
pub struct Animation {
    pub name: String,
    pub channels: Vec<Channel>,
}

impl Animation {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn duration(&self) -> f32 {
        self.channels
            .iter()
            .map(|channel| channel.track.duration())
            .fold(0.0, f32::max)
    }

    pub fn apply(&self, time: f32, pose: &mut Pose) {
        for channel in &self.channels {
            let target = channel.target;
            match &channel.track {
                Track::Translation(curve) => {
                    if let Some(value) = curve.sample(time)
                        && let Some(local) = pose.local_mut(target)
                    {
                        local.translation = value;
                    }
                }
                Track::Rotation(curve) => {
                    if let Some(value) = curve.sample(time)
                        && let Some(local) = pose.local_mut(target)
                    {
                        local.rotation = value.normalize();
                    }
                }
                Track::Scale(curve) => {
                    if let Some(value) = curve.sample(time)
                        && let Some(local) = pose.local_mut(target)
                    {
                        local.scale = value;
                    }
                }
                Track::Weights(weights) => {
                    if let Some(slot) = pose.weights_mut(target) {
                        if slot.len() < weights.count {
                            slot.resize(weights.count, 0.0);
                        }
                        weights.sample_into(time, slot);
                    }
                }
            }
        }
    }

    pub(crate) fn validate(&self, nodes: usize) -> Result<()> {
        for channel in &self.channels {
            if channel.target.index() >= nodes {
                return Err(Error::Invalid(format!(
                    "animation \"{}\" targets missing {}",
                    self.name, channel.target
                )));
            }
            channel
                .track
                .validate(&format!("track of animation \"{}\"", self.name))?;
        }
        Ok(())
    }
}

struct Span {
    start: usize,
    end: usize,
    factor: f32,
}

fn ascending(times: &[f32], what: &str) -> Result<()> {
    if times.iter().any(|time| !time.is_finite()) {
        return Err(Error::Invalid(format!(
            "{what} has a keyframe time that is not finite"
        )));
    }
    match times.windows(2).all(|pair| pair[0] <= pair[1]) {
        true => Ok(()),
        false => Err(Error::Invalid(format!(
            "{what} has decreasing keyframe times"
        ))),
    }
}
fn span(times: &[f32], time: f32) -> Option<Span> {
    let last = times.len().checked_sub(1)?;
    let clamped = |index: usize| Span {
        start: index,
        end: index,
        factor: 0.0,
    };
    if time.is_nan() || time <= times[0] {
        return Some(clamped(0));
    }
    if time >= times[last] {
        return Some(clamped(last));
    }
    let end = times.partition_point(|&key| key <= time).clamp(1, last);
    let start = end - 1;
    let width = times[end] - times[start];
    Some(Span {
        start,
        end,
        factor: match width > 0.0 {
            true => (time - times[start]) / width,
            false => 0.0,
        },
    })
}

fn hermite<T: Interpolate>(from: T, out_tangent: T, to: T, in_tangent: T, t: f32) -> T {
    let (t2, t3) = (t * t, t * t * t);
    from.scaled(2.0 * t3 - 3.0 * t2 + 1.0)
        .combined(out_tangent.scaled(t3 - 2.0 * t2 + t))
        .combined(to.scaled(-2.0 * t3 + 3.0 * t2))
        .combined(in_tangent.scaled(t3 - t2))
        .normalized()
}
