use std::f32::consts::TAU;

use rikka_model_core::glam::{Quat, Vec3};

use crate::part::{PartKey, Tag};
use crate::slopes::{Slope, Slopes};
use crate::tuning::{
    FLAT_TURN, FRAMES_PER_SECOND, HIT_SLOPE, SETTLE_DECAY, SETTLE_PERIOD, SETTLE_TAIL,
};

const MONOTONE_BOUND: f32 = 3.0;

#[derive(Clone, Copy, Debug)]
enum Place {
    Before,
    Between { at: usize, s: f32 },
    After { seconds: f32 },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Curve<'a> {
    keys: &'a [PartKey],
    place: Place,
}

impl<'a> Curve<'a> {
    pub(crate) fn locate(keys: &'a [PartKey], frame: f32) -> Option<Self> {
        let first = keys.first()?;
        let last = keys.last()?;
        let place = if frame <= first.frame as f32 {
            Place::Before
        } else if frame >= last.frame as f32 {
            Place::After {
                seconds: (frame - last.frame as f32) / FRAMES_PER_SECOND,
            }
        } else {
            let at = keys.partition_point(|key| key.frame as f32 <= frame) - 1;
            let width = (keys[at + 1].frame - keys[at].frame) as f32;
            Place::Between {
                at,
                s: (frame - keys[at].frame as f32) / width,
            }
        };
        Some(Self { keys, place })
    }

    fn segment(&self) -> usize {
        match self.place {
            Place::Before => 0,
            Place::Between { at, .. } => at,
            Place::After { .. } => self.keys.len() - 1,
        }
    }

    pub(crate) fn held_key(&self) -> &'a PartKey {
        &self.keys[self.segment()]
    }

    pub(crate) fn window(&self) -> &'a [PartKey] {
        let index = self.segment();
        &self.keys[index.saturating_sub(1)..(index + 3).min(self.keys.len())]
    }

    pub(crate) fn scalar(
        &self,
        value: impl Fn(&PartKey) -> f32,
        slope: impl Fn(&Slopes) -> Option<Slope>,
    ) -> f32 {
        self.interpolate(|index| value(&self.keys[index]), slope)
    }

    fn interpolate(
        &self,
        value: impl Fn(usize) -> f32,
        slope: impl Fn(&Slopes) -> Option<Slope>,
    ) -> f32 {
        let rule = Rule {
            keys: self.keys,
            value,
            slope,
        };
        match self.place {
            Place::Before => rule.value(0),
            Place::Between { at, s } => rule.span(at, s),
            Place::After { seconds } => {
                let last = self.keys.len() - 1;
                rule.value(last) + rule.settle(last, seconds, SETTLE_TAIL)
            }
        }
    }

    pub(crate) fn blend(&self, flag: impl Fn(&PartKey) -> bool) -> f32 {
        let weight = |key: &PartKey| if flag(key) { 1.0 } else { 0.0 };
        match self.place {
            Place::Before | Place::After { .. } => weight(self.held_key()),
            Place::Between { at, s } => {
                let (from, to) = (weight(&self.keys[at]), weight(&self.keys[at + 1]));
                from + (to - from) * smoothstep(s)
            }
        }
    }

    pub(crate) fn rotation(
        &self,
        value: impl Fn(&PartKey) -> Quat,
        slope: impl Fn(&Slopes) -> Option<Slope>,
    ) -> Quat {
        let current = self.segment();
        let low = current.saturating_sub(1);
        let window = self.window();
        let mut turns = [Quat::IDENTITY; 4];
        let mut arcs = [Arc::FLAT; 4];
        let mut angles = [0.0; 4];
        for (slot, key) in window.iter().enumerate() {
            let turn = value(key).normalize();
            turns[slot] = match slot {
                0 => turn,
                _ if turns[slot - 1].dot(turn) < 0.0 => -turn,
                _ => turn,
            };
            if slot > 0 {
                arcs[slot - 1] = Arc::from_rotations(turns[slot - 1], turns[slot]);
                angles[slot] = angles[slot - 1] + arcs[slot - 1].angle;
            }
        }
        let angle = self.interpolate(|index| angles[index - low], slope);
        let (from, arc) = match self.place {
            Place::Before => return turns[0],
            Place::Between { at, .. } => (at - low, arcs[at - low]),
            Place::After { .. } if current > 0 => (current - low, arcs[current - low - 1]),
            Place::After { .. } => return turns[0],
        };
        if arc.angle <= FLAT_TURN {
            return turns[from];
        }
        let travelled = angle - angles[from];
        (turns[from] * Quat::from_axis_angle(arc.axis, travelled)).normalize()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Arc {
    pub(crate) axis: Vec3,
    pub(crate) angle: f32,
}

impl Arc {
    const FLAT: Self = Self {
        axis: Vec3::X,
        angle: 0.0,
    };

    pub(crate) fn from_rotations(from: Quat, to: Quat) -> Self {
        let relative = from.conjugate() * to;
        let sine = relative.xyz().length();
        if sine <= FLAT_TURN {
            return Self::FLAT;
        }
        Self {
            axis: relative.xyz() / sine,
            angle: 2.0 * sine.atan2(relative.w),
        }
    }
}

struct Rule<'a, V, S> {
    keys: &'a [PartKey],
    value: V,
    slope: S,
}

impl<V: Fn(usize) -> f32, S: Fn(&Slopes) -> Option<Slope>> Rule<'_, V, S> {
    fn value(&self, index: usize) -> f32 {
        (self.value)(index)
    }

    fn explicit(&self, index: usize) -> Option<Slope> {
        self.keys[index].slopes.as_ref().and_then(&self.slope)
    }

    fn tag(&self, index: usize) -> Tag {
        self.keys[index].tag
    }

    fn width(&self, index: usize) -> f32 {
        (self.keys[index + 1].frame - self.keys[index].frame) as f32
    }

    fn secant(&self, index: usize) -> f32 {
        (self.value(index + 1) - self.value(index)) / self.width(index)
    }

    fn steffen(&self, index: usize) -> f32 {
        if index == 0 || index + 1 >= self.keys.len() {
            return 0.0;
        }
        let (before, after) = (self.secant(index - 1), self.secant(index));
        let (wide_before, wide_after) = (self.width(index - 1), self.width(index));
        let parabola = (before * wide_after + after * wide_before) / (wide_before + wide_after);
        (sign(before) + sign(after)) * before.abs().min(after.abs()).min(0.5 * parabola.abs())
    }

    fn incoming(&self, index: usize) -> f32 {
        if index == 0 {
            return 0.0;
        }
        match self.tag(index) {
            Tag::Smooth | Tag::Step => self.steffen(index),
            Tag::Ease => 0.0,
            Tag::Hit => {
                let secant = self.secant(index - 1);
                let bound = MONOTONE_BOUND * secant;
                (HIT_SLOPE * secant).clamp(bound.min(0.0), bound.max(0.0))
            }
            Tag::Linear | Tag::Settle => self.secant(index - 1),
        }
    }

    fn outgoing(&self, index: usize) -> f32 {
        match self.tag(index) {
            Tag::Smooth => self.steffen(index),
            Tag::Linear => self.secant(index),
            Tag::Ease | Tag::Hit | Tag::Step | Tag::Settle => 0.0,
        }
    }

    fn span(&self, at: usize, s: f32) -> f32 {
        let leaving = self.explicit(at);
        if leaving.is_none() && self.tag(at) == Tag::Step {
            return self.value(at);
        }
        let width = self.width(at);
        let arriving = self
            .explicit(at + 1)
            .map_or_else(|| self.incoming(at + 1), |slope| slope.arriving);
        let leaving = leaving.map_or_else(|| self.outgoing(at), |slope| slope.leaving);
        let eased = hermite(
            self.value(at),
            self.value(at + 1),
            leaving * width,
            arriving * width,
            s,
        );
        let seconds = width / FRAMES_PER_SECOND;
        eased + self.settle(at, s * seconds, seconds)
    }

    fn settle(&self, at: usize, seconds: f32, fade: f32) -> f32 {
        if self.tag(at) != Tag::Settle || self.explicit(at).is_some() || at == 0 || seconds >= fade
        {
            return 0.0;
        }
        let speed = self.incoming(at) * FRAMES_PER_SECOND;
        let amplitude = speed * SETTLE_PERIOD / TAU;
        amplitude
            * (-seconds / SETTLE_DECAY).exp()
            * (TAU * seconds / SETTLE_PERIOD).sin()
            * (1.0 - smoothstep(seconds / fade))
    }
}

fn sign(value: f32) -> f32 {
    match value {
        value if value > 0.0 => 1.0,
        value if value < 0.0 => -1.0,
        _ => 0.0,
    }
}

fn hermite(from: f32, to: f32, leaving: f32, arriving: f32, s: f32) -> f32 {
    let (square, cube) = (s * s, s * s * s);
    (2.0 * cube - 3.0 * square + 1.0) * from
        + (cube - 2.0 * square + s) * leaving
        + (-2.0 * cube + 3.0 * square) * to
        + (cube - square) * arriving
}

pub(crate) fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
