use animate_body::{muscles, twist_about};
use rikka_model_core::glam::Quat;

use crate::curve::{Arc, Curve};
use crate::part::{Part, PartKey, PartValues, Tag};
use crate::sample::sampled_values;
use crate::slopes::{Slope, Slopes};
use crate::tuning::{FIT_RIDGE, FLAT_TURN, REDUCE_DEGREES, REDUCE_POSITION};

const FALLBACKS: [Tag; 5] = [Tag::Smooth, Tag::Linear, Tag::Ease, Tag::Hit, Tag::Step];
const CURVE_BEHIND: usize = 2;

struct Channels {
    scalars: Vec<Vec<f32>>,
    turns: Option<Vec<Quat>>,
}

impl Channels {
    fn from_values(values: &[PartValues]) -> Self {
        let count = values.first().map_or(0, PartValues::scalar_count);
        let scalars = (0..count)
            .map(|channel| values.iter().map(|values| values.scalar(channel)).collect())
            .collect();
        let turns = values.first().and_then(PartValues::turn).map(|_| {
            values
                .iter()
                .map(|values| values.turn().unwrap_or_default().normalize())
                .collect()
        });
        Self { scalars, turns }
    }

    fn unset(&self) -> Slopes {
        Slopes {
            scalars: vec![Slope::default(); self.scalars.len()],
            turn: self.turns.as_ref().map(|_| Slope::default()),
        }
    }

    fn fit(&self, from: u32, to: u32, leaving: &mut Slopes, arriving: &mut Slopes) {
        let set = |fitted: Fitted, out: &mut Slope, into: &mut Slope| {
            out.leaving = fitted.leaving;
            into.arriving = fitted.arriving;
        };
        for (channel, series) in self.scalars.iter().enumerate() {
            set(
                fitted(|frame| series[frame as usize], from, to),
                &mut leaving.scalars[channel],
                &mut arriving.scalars[channel],
            );
        }
        if let (Some(turns), Some(out), Some(into)) =
            (&self.turns, leaving.turn.as_mut(), arriving.turn.as_mut())
        {
            set(swept(turns, from, to), out, into);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Fitted {
    leaving: f32,
    arriving: f32,
}

fn fitted(series: impl Fn(u32) -> f32, from: u32, to: u32) -> Fitted {
    let width = f64::from(to - from);
    let (start, end) = (f64::from(series(from)), f64::from(series(to)));
    let secant = (end - start) / width;
    let ridge = f64::from(FIT_RIDGE);
    let (mut pp, mut pq, mut qq) = (ridge, 0.0, ridge);
    let (mut pr, mut qr) = (ridge * secant, ridge * secant);
    for frame in from + 1..to {
        let s = f64::from(frame - from) / width;
        let (square, cube) = (s * s, s * s * s);
        let base = (2.0 * cube - 3.0 * square + 1.0) * start + (3.0 * square - 2.0 * cube) * end;
        let leaving = (cube - 2.0 * square + s) * width;
        let arriving = (cube - square) * width;
        let rest = f64::from(series(frame)) - base;
        pp += leaving * leaving;
        pq += leaving * arriving;
        qq += arriving * arriving;
        pr += leaving * rest;
        qr += arriving * rest;
    }
    let determinant = pp * qq - pq * pq;
    Fitted {
        leaving: ((pr * qq - qr * pq) / determinant) as f32,
        arriving: ((pp * qr - pq * pr) / determinant) as f32,
    }
}

fn swept(turns: &[Quat], from: u32, to: u32) -> Fitted {
    let start = turns[from as usize];
    let end = match start.dot(turns[to as usize]) < 0.0 {
        true => -turns[to as usize],
        false => turns[to as usize],
    };
    let arc = Arc::from_rotations(start, end);
    if arc.angle <= FLAT_TURN {
        return Fitted {
            leaving: 0.0,
            arriving: 0.0,
        };
    }
    let inverse = start.conjugate();
    let along = |frame: u32| match frame {
        _ if frame == from => 0.0,
        _ if frame == to => arc.angle,
        _ => twist_about(inverse * turns[frame as usize], arc.axis),
    };
    fitted(along, from, to)
}

struct Fitting<'a> {
    part: Part,
    values: &'a [PartValues],
    pinned: &'a [bool],
    channels: Channels,
    keys: Vec<PartKey>,
    errors: Vec<f32>,
}

impl Fitting<'_> {
    fn key(&self, frame: u32) -> PartKey {
        PartKey {
            part: self.part,
            frame,
            tag: Tag::Smooth,
            values: self.values[frame as usize].clone(),
            slopes: Some(self.channels.unset()),
        }
    }

    fn fit(&mut self, at: usize) {
        let (from, to) = (self.keys[at].frame, self.keys[at + 1].frame);
        let (before, after) = self.keys.split_at_mut(at + 1);
        if let (Some(leaving), Some(arriving)) =
            (before[at].slopes.as_mut(), after[0].slopes.as_mut())
        {
            self.channels.fit(from, to, leaving, arriving);
        }
    }

    fn error_at(&self, frame: u32) -> f32 {
        self.error_within(0, frame)
    }

    fn error_within(&self, first: usize, frame: u32) -> f32 {
        let at = frame as usize;
        Curve::locate(&self.keys[first..], frame as f32).map_or(0.0, |curve| {
            mismatch(
                self.part,
                &sampled_values(&curve),
                &self.values[at],
                self.pinned.get(at).copied().unwrap_or(false),
            )
        })
    }

    fn measure(&mut self, from: u32, to: u32) -> f32 {
        self.measure_within(0, from, to)
    }

    fn measure_within(&mut self, first: usize, from: u32, to: u32) -> f32 {
        let mut worst: f32 = 0.0;
        for frame in from..=to {
            let error = self.error_within(first, frame);
            self.errors[frame as usize] = error;
            worst = worst.max(error);
        }
        worst
    }

    fn insert(&mut self, frame: u32) {
        let at = self.keys.partition_point(|key| key.frame < frame);
        let key = self.key(frame);
        self.keys.insert(at, key);
        self.fit(at - 1);
        self.fit(at);
        self.measure(self.keys[at - 1].frame, self.keys[at + 1].frame);
    }

    fn spare_all(&mut self, forced: &[u32]) {
        let mut alive = vec![true; self.keys.len()];
        let mut behind = 1;
        for at in 1..self.keys.len().saturating_sub(1) {
            let carried = behind.min(CURVE_BEHIND);
            match !forced.contains(&self.keys[at].frame) && self.spare(at, carried) {
                true => {
                    alive[at] = false;
                    alive[at - carried..=at].rotate_right(1);
                    behind = carried;
                }
                false => behind += 1,
            }
        }
        let mut alive = alive.into_iter();
        self.keys.retain(|_| alive.next().unwrap_or(true));
    }

    fn spare(&mut self, at: usize, carried: usize) -> bool {
        let (from, to) = (self.keys[at - 1].frame, self.keys[at + 1].frame);
        let neighbour_slopes = (
            self.keys[at - 1].slopes.clone(),
            self.keys[at + 1].slopes.clone(),
        );
        let errors = self.errors[from as usize..=to as usize].to_vec();
        let start = at - carried;
        self.keys[start..=at].rotate_right(1);
        self.fit(at);
        if self.measure_within(start + 1, from, to) <= 1.0 {
            return true;
        }
        self.keys[start..=at].rotate_left(1);
        self.keys[at - 1].slopes = neighbour_slopes.0;
        self.keys[at + 1].slopes = neighbour_slopes.1;
        self.errors[from as usize..=to as usize].copy_from_slice(&errors);
        false
    }

    fn fallback(&mut self, at: usize) {
        let from = self.keys[at.saturating_sub(1)].frame;
        let to = self.keys[(at + 1).min(self.keys.len() - 1)].frame;
        let slopes = self.keys[at].slopes.take();
        let mut best = (f32::MAX, Tag::Smooth);
        for tag in FALLBACKS {
            self.keys[at].tag = tag;
            let cost: f32 = (from..=to).map(|frame| self.error_at(frame).powi(2)).sum();
            if cost < best.0 {
                best = (cost, tag);
            }
        }
        self.keys[at].tag = best.1;
        self.keys[at].slopes = slopes;
    }
}

pub(super) fn reduce(
    part: Part,
    values: &[PartValues],
    forced: &[u32],
    pinned: &[bool],
    seeds: &[u32],
) -> Vec<PartKey> {
    let Some(last) = values.len().checked_sub(1) else {
        return Vec::new();
    };
    let last = last as u32;
    let mut frames: Vec<u32> = forced
        .iter()
        .chain(seeds)
        .copied()
        .filter(|frame| *frame <= last)
        .chain([0, last])
        .collect();
    frames.sort_unstable();
    frames.dedup();
    let mut fitting = Fitting {
        part,
        values,
        pinned,
        channels: Channels::from_values(values),
        keys: Vec::new(),
        errors: vec![0.0; values.len()],
    };
    fitting.keys = frames.into_iter().map(|frame| fitting.key(frame)).collect();
    for at in 0..fitting.keys.len() - 1 {
        fitting.fit(at);
    }
    fitting.measure(0, last);
    while let Some((worst, _)) = fitting
        .errors
        .iter()
        .copied()
        .enumerate()
        .max_by(|one, two| one.1.total_cmp(&two.1))
        .filter(|(_, error)| *error > 1.0)
    {
        fitting.insert(worst as u32);
    }
    fitting.spare_all(forced);
    for at in 0..fitting.keys.len() {
        fitting.fallback(at);
    }
    fitting.keys
}

fn mismatch(part: Part, pose_sample: &PartValues, original: &PartValues, pinned: bool) -> f32 {
    let angle = REDUCE_DEGREES.to_radians();
    let muscled = |one: &[f32], two: &[f32]| {
        let start = part.muscles().map_or(0, |range| range.start);
        one.iter()
            .zip(two)
            .enumerate()
            .map(|(at, (one, two))| {
                let muscle = muscles()[start + at];
                (muscle.angle(*one) - muscle.angle(*two)).abs() / angle
            })
            .fold(0.0, f32::max)
    };
    match (pose_sample, original) {
        (
            PartValues::Root { position, rotation },
            PartValues::Root {
                position: desired,
                rotation: turned,
            },
        ) => (position.distance(*desired) / REDUCE_POSITION)
            .max(rotation.angle_between(*turned) / angle),
        (PartValues::Muscles(one), PartValues::Muscles(two)) => muscled(one, two),
        (
            PartValues::Limb {
                muscles: one, goal, ..
            },
            PartValues::Limb {
                muscles: two,
                goal: desired,
                ..
            },
        ) => {
            let error = muscled(one, two);
            match pinned {
                true => error
                    .max(goal.position.distance(desired.position) / REDUCE_POSITION)
                    .max(goal.rotation.angle_between(desired.rotation) / angle),
                false => error,
            }
        }
        (PartValues::Gaze { eyes: one, .. }, PartValues::Gaze { eyes: two, .. }) => {
            muscled(one, two)
        }
        _ => 0.0,
    }
}
