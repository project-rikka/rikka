use std::ops::Range;

use rikka_types::Point;

use super::cubic;
use super::key::{Bend, Handle, Incoming, Key, Outgoing, Preset, Smooth, Timing};
use super::kind::Kind;
use super::leg::{Leg, Shape};
use super::route::{Points, Route};
use super::track::Track;

const FLAT: f64 = 1e-6;
const NOWHERE: f64 = 1e-12;

impl<K: Kind> Track<K> {
    pub(crate) fn cropped(&self, span: &Range<i64>) -> Track<K> {
        if self.is_empty() || span.is_empty() {
            return Track::default();
        }
        let cropped = tail(head(self.keys().to_vec(), span.start), span.end - 1);
        Track::from_normalized(
            cropped
                .into_iter()
                .map(|key| Key {
                    at: key.at - span.start,
                    ..key
                })
                .collect(),
        )
    }
}

fn head<K: Kind>(keys: Vec<Key<K>>, first: i64) -> Vec<Key<K>> {
    let after = keys.partition_point(|key| key.at < first);
    if after == 0 {
        return keys;
    }
    let Some(next) = keys.get(after) else {
        return vec![Key::new(first, keys[keys.len() - 1].value)];
    };
    if next.at == first {
        let mut rest = keys[after..].to_vec();
        freeze(&mut rest[0]);
        return rest;
    }
    let from = &keys[after - 1];
    let cut = first - from.at;
    let frames = next.at - from.at;
    let mut cuts = vec![cut];
    cuts.extend(steadied(from, frames, (cut, frames), cut + 1..frames));
    let mut tail_keys = divide(from, next, &cuts).split_off(1);
    tail_keys[0].timing.incoming = Incoming::Linear;
    tail_keys.extend_from_slice(&keys[after + 1..]);
    tail_keys
}

fn tail<K: Kind>(keys: Vec<Key<K>>, last: i64) -> Vec<Key<K>> {
    let after = keys.partition_point(|key| key.at <= last);
    if after == keys.len() {
        return keys;
    }
    if after == 0 {
        return vec![Key::new(last, keys[0].value)];
    }
    let from = &keys[after - 1];
    if from.at == last {
        let mut head_keys = keys[..after].to_vec();
        if let Some(end) = head_keys.last_mut() {
            freeze(end);
        }
        return head_keys;
    }
    let next = &keys[after];
    let cut = last - from.at;
    let frames = next.at - from.at;
    let mut cuts: Vec<i64> = steadied(from, frames, (0, cut), (1..cut).rev())
        .into_iter()
        .collect();
    cuts.push(cut);
    let mut divided = divide(from, next, &cuts);
    divided.pop();
    if let Some(end) = divided.last_mut() {
        end.timing.outgoing = Outgoing::Linear;
    }
    let mut head_keys = keys[..after - 1].to_vec();
    head_keys.extend(divided);
    head_keys
}

fn steadied<K: Kind>(
    from: &Key<K>,
    frames: i64,
    span: (i64, i64),
    mut candidates: impl Iterator<Item = i64>,
) -> Option<i64> {
    let Outgoing::Preset(preset) = from.timing.outgoing else {
        return None;
    };
    let eased = |frame: i64| {
        let progress = frame as f64 / frames as f64;
        let at =
            f64::from(preset.from) + (f64::from(preset.to) - f64::from(preset.from)) * progress;
        preset.easing.at(at)
    };
    let steady = |left: i64, right: i64| (eased(right) - eased(left)).abs() >= FLAT;
    let (start, end) = span;
    if steady(start, end) {
        return None;
    }
    candidates.find(|middle| steady(start, *middle) && steady(*middle, end))
}

fn freeze<K: Kind>(key: &mut Key<K>) {
    if key.timing.smooth == Smooth::Auto {
        key.timing.smooth = Smooth::Continuous;
    }
    if let Some(bend) = K::bend_mut(&mut key.bend)
        && bend.smooth == Smooth::Auto
    {
        bend.smooth = Smooth::Continuous;
    }
}

fn divide<K: Kind>(from: &Key<K>, to: &Key<K>, cuts: &[i64]) -> Vec<Key<K>> {
    let leg = Leg::between(from, to);
    let times: Vec<f64> = std::iter::once(0.0)
        .chain(cuts.iter().map(|cut| *cut as f64))
        .chain(std::iter::once(leg.frames))
        .collect();
    let last = times.len() - 1;
    let mut keys: Vec<Key<K>> = times
        .iter()
        .enumerate()
        .map(|(index, time)| match index {
            0 => *from,
            index if index == last => *to,
            _ => Key {
                timing: Timing::LINEAR,
                bend: inner_bend::<K>(),
                ..Key::new(from.at + *time as i64, leg.value(from, to, *time))
            },
        })
        .collect();
    freeze(&mut keys[0]);
    freeze(&mut keys[last]);
    let travels: Vec<f64> = times
        .iter()
        .enumerate()
        .map(|(index, time)| match index {
            0 => 0.0,
            index if index == last => leg.route.length(),
            _ => leg.travel(*time),
        })
        .collect();
    timed(&mut keys, &leg, &times, &travels, from, to);
    if K::bend(&from.bend).is_some() {
        bent(&mut keys, &leg.route, &travels);
    }
    keys
}

fn inner_bend<K: Kind>() -> K::Bend {
    let mut bend = K::Bend::default();
    if let Some(spatial) = K::bend_mut(&mut bend) {
        *spatial = Bend {
            incoming: Point::ZERO,
            outgoing: Point::ZERO,
            smooth: Smooth::Manual,
            straight: false,
        };
    }
    bend
}

fn timed<K: Kind>(
    keys: &mut [Key<K>],
    leg: &Leg,
    times: &[f64],
    travels: &[f64],
    from: &Key<K>,
    to: &Key<K>,
) {
    let last = keys.len() - 1;
    match leg.shape {
        Shape::Hold => {
            for key in &mut keys[..last] {
                key.timing.outgoing = Outgoing::Hold;
            }
        }
        Shape::Preset(preset) => {
            let at = |time: f64| {
                (f64::from(preset.from)
                    + (f64::from(preset.to) - f64::from(preset.from)) * time / leg.frames)
                    as f32
            };
            let marks: Vec<f32> = times
                .iter()
                .enumerate()
                .map(|(index, time)| match index {
                    0 => preset.from,
                    index if index == last => preset.to,
                    _ => at(*time),
                })
                .collect();
            for (index, key) in keys[..last].iter_mut().enumerate() {
                key.timing.outgoing = Outgoing::Preset(Preset {
                    from: marks[index],
                    to: marks[index + 1],
                    ..preset
                });
            }
        }
        Shape::Curve { xs, ys } => {
            let linear = matches!(from.timing.outgoing, Outgoing::Linear)
                && matches!(to.timing.incoming, Incoming::Linear);
            if linear {
                return;
            }
            let params: Vec<f64> = times
                .iter()
                .enumerate()
                .map(|(index, time)| match index {
                    0 => 0.0,
                    index if index == last => 1.0,
                    _ => cubic::solve(xs, *time),
                })
                .collect();
            for index in 0..last {
                let (span_x, span_y) = (
                    cubic::segment(xs, params[index], params[index + 1]),
                    cubic::segment(ys, params[index], params[index + 1]),
                );
                let frames = times[index + 1] - times[index];
                let sign = match K::DIMENSIONS {
                    1 => 1.0,
                    _ if travels[index + 1] < travels[index] => -1.0,
                    _ => 1.0,
                };
                let handle = |across: f64, rise: f64| {
                    let speed = match across > NOWHERE {
                        true => sign * rise / across,
                        false => 0.0,
                    };
                    Handle::new(speed as f32, (across / frames) as f32).checked()
                };
                keys[index].timing.outgoing =
                    Outgoing::Bezier(handle(span_x[1] - span_x[0], span_y[1] - span_y[0]));
                keys[index + 1].timing.incoming =
                    Incoming::Bezier(handle(span_x[3] - span_x[2], span_y[3] - span_y[2]));
            }
        }
    }
}

fn bent<K: Kind>(keys: &mut [Key<K>], route: &Route, travels: &[f64]) {
    let last = keys.len() - 1;
    match route {
        Route::Curve(arc) => {
            let length = route.length();
            for index in 0..last {
                let (start, end) = (
                    travels[index].clamp(0.0, length),
                    travels[index + 1].clamp(0.0, length),
                );
                let piece = match start <= end {
                    true => sub_arc(arc.points(), arc.param(start), arc.param(end)),
                    false => {
                        let reversed = sub_arc(arc.points(), arc.param(end), arc.param(start));
                        [reversed[3], reversed[2], reversed[1], reversed[0]]
                    }
                };
                set_tangent(&mut keys[index], piece[1], piece[0], false);
                set_tangent(&mut keys[index + 1], piece[2], piece[3], true);
            }
        }
        Route::Line { length, .. } => {
            let whole = length.abs();
            let share = |index: usize| match whole > NOWHERE {
                true => ((travels[index + 1] - travels[index]).abs() / whole) as f32,
                false => 0.0,
            };
            let (first, end) = (share(0), share(last - 1));
            scale_tangent(&mut keys[0], first, false);
            scale_tangent(&mut keys[last], end, true);
        }
    }
}

fn sub_arc(points: Points, start: f64, end: f64) -> Points {
    let xs = cubic::segment(points.map(|point| point[0]), start, end);
    let ys = cubic::segment(points.map(|point| point[1]), start, end);
    std::array::from_fn(|index| [xs[index], ys[index]])
}

fn set_tangent<K: Kind>(key: &mut Key<K>, handle: [f64; 2], base: [f64; 2], incoming: bool) {
    let Some(bend) = K::bend_mut(&mut key.bend) else {
        return;
    };
    if bend.straight {
        return;
    }
    let tangent = Point::new((handle[0] - base[0]) as f32, (handle[1] - base[1]) as f32);
    match incoming {
        true => bend.incoming = tangent,
        false => bend.outgoing = tangent,
    }
}

fn scale_tangent<K: Kind>(key: &mut Key<K>, share: f32, incoming: bool) {
    let Some(bend) = K::bend_mut(&mut key.bend) else {
        return;
    };
    match incoming {
        true => bend.incoming = bend.incoming * share,
        false => bend.outgoing = bend.outgoing * share,
    }
}
