use std::f32::consts::{FRAC_PI_2, PI};

use rikka_model_core::glam::Vec2;

use crate::file::Record;

const MUTED: i64 = 1 << 4;
const DISABLED: i64 = 1 << 10;
const INTEGER_VALUES: i64 = 1 << 11;
const DISCRETE_VALUES: i64 = 1 << 12;
const EXTRAPOLATE_CONSTANT: i64 = 0;
const MODIFIER_CYCLES: i64 = 4;
const MODIFIER_DISABLED: i64 = 1;
const MODIFIER_MUTED: i64 = 1 << 3;
const CYCLIC_OFFSET: i64 = 2;
const CYCLIC_MIRROR: i64 = 3;

const CONSTANT: i64 = 0;
const LINEAR: i64 = 1;
const BEZIER: i64 = 2;
const BACK: i64 = 3;
const BOUNCE: i64 = 4;
const CIRCULAR: i64 = 5;
const CUBIC: i64 = 6;
const ELASTIC: i64 = 7;
const EXPONENTIAL: i64 = 8;
const QUADRATIC: i64 = 9;
const QUARTIC: i64 = 10;
const QUINTIC: i64 = 11;
const SINE: i64 = 12;
const EASE_IN: i64 = 1;
const EASE_OUT: i64 = 2;
const EASE_IN_OUT: i64 = 3;

#[derive(Clone, Copy, Debug)]
struct Key {
    left: Vec2,
    point: Vec2,
    right: Vec2,
    interpolation: i64,
    easing: i64,
    back: f32,
    amplitude: f32,
    period: f32,
}

#[derive(Clone, Copy, Debug)]
struct Cycles {
    before: i64,
    after: i64,
    before_count: i64,
    after_count: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct FCurve {
    pub(crate) path: String,
    pub(crate) index: i64,
    keys: Vec<Key>,
    samples: Vec<Vec2>,
    extrapolate_constant: bool,
    discrete: bool,
    integer: bool,
    cycles: Option<Cycles>,
    pub(crate) unsupported_modifiers: bool,
}

impl FCurve {
    pub(crate) fn read(curve: &Record<'_>) -> Option<Self> {
        let flag = curve.int("flag").unwrap_or(0);
        if flag & (MUTED | DISABLED) != 0 {
            return None;
        }
        let path = curve.string("rna_path")?;
        let count = curve.int("totvert").unwrap_or(0).max(0) as usize;
        let keys: Vec<Key> = curve
            .table("bezt", "BezTriple")
            .map(|table| {
                table
                    .rows()
                    .take(count)
                    .filter_map(|row| {
                        let vec = row.floats::<9>("vec")?;
                        Some(Key {
                            left: Vec2::new(vec[0], vec[1]),
                            point: Vec2::new(vec[3], vec[4]),
                            right: Vec2::new(vec[6], vec[7]),
                            interpolation: row.int("ipo").unwrap_or(BEZIER),
                            easing: row.int("easing").unwrap_or(0),
                            back: row.float("back").unwrap_or(1.70158),
                            amplitude: row.float("amplitude").unwrap_or(0.8),
                            period: row.float("period").unwrap_or(4.1),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let samples: Vec<Vec2> = match keys.is_empty() {
            true => curve
                .table("fpt", "FPoint")
                .and_then(|table| table.floats::<2>("vec"))
                .map(|values| {
                    values
                        .into_iter()
                        .take(count)
                        .map(Vec2::from_array)
                        .collect()
                })
                .unwrap_or_default(),
            false => Vec::new(),
        };
        if keys.is_empty() && samples.is_empty() {
            return None;
        }
        let mut cycles = None;
        let mut unsupported_modifiers = false;
        for (position, modifier) in curve.list("modifiers").iter().enumerate() {
            let modifier_flag = modifier.int("flag").unwrap_or(0);
            if modifier_flag & (MODIFIER_DISABLED | MODIFIER_MUTED) != 0 {
                continue;
            }
            match (modifier.int("type"), position) {
                (Some(MODIFIER_CYCLES), 0) => {
                    cycles = modifier.deref("data").map(|data| Cycles {
                        before: data.int("before_mode").unwrap_or(1),
                        after: data.int("after_mode").unwrap_or(1),
                        before_count: data.int("before_cycles").unwrap_or(0),
                        after_count: data.int("after_cycles").unwrap_or(0),
                    });
                }
                _ => unsupported_modifiers = true,
            }
        }
        Some(Self {
            path,
            index: curve.int("array_index").unwrap_or(0),
            keys,
            samples,
            extrapolate_constant: curve.int("extend").unwrap_or(EXTRAPOLATE_CONSTANT)
                == EXTRAPOLATE_CONSTANT,
            discrete: flag & DISCRETE_VALUES != 0,
            integer: flag & INTEGER_VALUES != 0,
            cycles,
            unsupported_modifiers,
        })
    }

    pub(crate) fn evaluate(&self, frame: f32) -> f32 {
        let (time, offset) = self.cycle(frame);
        let mut value = match self.keys.is_empty() {
            true => self.sampled(time),
            false => self.keyed(time),
        } + offset;
        if self.integer || self.discrete {
            value = (value + 0.5).floor();
        }
        value
    }

    pub(crate) fn breaks(&self, from: f32, to: f32, each: &mut dyn FnMut(f32) -> bool) {
        let points = self
            .keys
            .iter()
            .map(|key| key.point.x)
            .chain(self.samples.iter().map(|sample| sample.x));
        for at in points.clone() {
            if (from..=to).contains(&at) && !each(at) {
                return;
            }
        }
        let (first, last) = self.ends();
        let width = f64::from(last.x - first.x);
        let Some(cycles) = self.cycles.filter(|_| width > 0.0) else {
            return;
        };
        let (from, to) = (f64::from(from), f64::from(to));
        let (first, last) = (f64::from(first.x), f64::from(last.x));
        let sides = [
            (
                -1.0,
                cycles.before,
                cycles.before_count,
                (first - to, last - from),
            ),
            (
                1.0,
                cycles.after,
                cycles.after_count,
                (from - last, to - first),
            ),
        ];
        for (side, mode, count, (near, far)) in sides {
            if mode == 0 {
                continue;
            }
            let lowest = (near / width).ceil().max(1.0);
            let mut highest = (far / width).floor();
            if count > 0 {
                highest = highest.min(count as f64);
            }
            let mut cycle = lowest;
            while cycle <= highest {
                let shift = side * cycle * width;
                let mirrored = mode == CYCLIC_MIRROR && cycle % 2.0 != 0.0;
                for at in points.clone() {
                    let at = f64::from(at);
                    let shifted = match mirrored {
                        true => first + shift + (last - at),
                        false => at + shift,
                    };
                    if (from..=to).contains(&shifted) && !each(shifted as f32) {
                        return;
                    }
                }
                cycle += 1.0;
            }
        }
    }

    fn ends(&self) -> (Vec2, Vec2) {
        match (self.keys.first(), self.keys.last()) {
            (Some(first), Some(last)) => (first.point, last.point),
            _ => (
                self.samples.first().copied().unwrap_or(Vec2::ZERO),
                self.samples.last().copied().unwrap_or(Vec2::ZERO),
            ),
        }
    }

    fn cycle(&self, frame: f32) -> (f32, f32) {
        let Some(cycles) = self.cycles else {
            return (frame, 0.0);
        };
        let (first, last) = self.ends();
        let (side, mode, count, origin) = if frame < first.x && cycles.before != 0 {
            (-1.0f64, cycles.before, cycles.before_count, first.x)
        } else if frame > last.x && cycles.after != 0 {
            (1.0f64, cycles.after, cycles.after_count, last.x)
        } else {
            return (frame, 0.0);
        };
        let width = last.x - first.x;
        let rise = last.y - first.y;
        if width == 0.0 {
            return (frame, 0.0);
        }
        let cycle = side * (f64::from(frame) - f64::from(origin)) / f64::from(width);
        let within = (frame - origin) % width;
        if count != 0 && cycle > count as f64 {
            return (frame, 0.0);
        }
        let offset = match mode == CYCLIC_OFFSET {
            true => {
                let steps = match side < 0.0 {
                    true => ((frame - origin) / width).floor(),
                    false => ((frame - origin) / width).ceil(),
                };
                steps * rise
            }
            false => 0.0,
        };
        let mut time = if within == 0.0 {
            let mut time = match side > 0.0 {
                true => last.x,
                false => first.x,
            };
            if mode == CYCLIC_MIRROR && (cycle as i64) % 2 != 0 {
                time = match side > 0.0 {
                    true => first.x,
                    false => last.x,
                };
            }
            time
        } else if mode == CYCLIC_MIRROR && ((cycle + 1.0) as i64) % 2 != 0 {
            match side < 0.0 {
                true => first.x - within,
                false => last.x - within,
            }
        } else {
            first.x + within
        };
        if time < first.x {
            time += width;
        }
        (time, offset)
    }

    fn sampled(&self, time: f32) -> f32 {
        let first = self.samples[0];
        let last = self.samples[self.samples.len() - 1];
        if time <= first.x {
            return first.y;
        }
        if time >= last.x {
            return last.y;
        }
        let at = time - first.x;
        let low = (at.floor() as usize).min(self.samples.len() - 1);
        let high = (low + 1).min(self.samples.len() - 1);
        let factor = at - at.floor();
        self.samples[low].y + (self.samples[high].y - self.samples[low].y) * factor
    }

    fn keyed(&self, time: f32) -> f32 {
        let first = self.keys[0];
        let last = self.keys[self.keys.len() - 1];
        if first.point.x >= time {
            return self.extrapolate(time, 0, true);
        }
        if last.point.x <= time {
            return self.extrapolate(time, self.keys.len() - 1, false);
        }
        self.interpolate(time)
    }

    fn extrapolate(&self, time: f32, end: usize, before: bool) -> f32 {
        let key = self.keys[end];
        if key.interpolation == CONSTANT || self.extrapolate_constant || self.discrete {
            return key.point.y;
        }
        if key.interpolation == LINEAR {
            if self.keys.len() == 1 {
                return key.point.y;
            }
            let neighbour = match before {
                true => self.keys[end + 1],
                false => self.keys[end - 1],
            };
            let run = neighbour.point.x - key.point.x;
            if run == 0.0 {
                return key.point.y;
            }
            let slope = (neighbour.point.y - key.point.y) / run;
            return key.point.y - slope * (key.point.x - time);
        }
        let handle = match before {
            true => key.left,
            false => key.right,
        };
        let run = key.point.x - handle.x;
        if run == 0.0 {
            return key.point.y;
        }
        let slope = (key.point.y - handle.y) / run;
        key.point.y - slope * (key.point.x - time)
    }

    fn interpolate(&self, time: f32) -> f32 {
        const THRESHOLD: f32 = 0.0001;
        let index = self
            .keys
            .partition_point(|key| key.point.x < time - THRESHOLD);
        let index = index.min(self.keys.len() - 1);
        let key = self.keys[index];
        if (key.point.x - time).abs() < THRESHOLD {
            return key.point.y;
        }
        let previous = self.keys[index.saturating_sub(1)];
        if time < previous.point.x || key.point.x < time {
            return 0.0;
        }
        let begin = previous.point.y;
        let change = key.point.y - previous.point.y;
        let duration = key.point.x - previous.point.x;
        let elapsed = time - previous.point.x;
        if previous.interpolation == CONSTANT || self.discrete || duration == 0.0 {
            return previous.point.y;
        }
        let ease = previous.easing;
        match previous.interpolation {
            BEZIER => bezier(previous, key, time),
            LINEAR => change * elapsed / duration + begin,
            BACK => match ease {
                EASE_IN => back_in(elapsed, begin, change, duration, previous.back),
                EASE_IN_OUT => back_in_out(elapsed, begin, change, duration, previous.back),
                _ => back_out(elapsed, begin, change, duration, previous.back),
            },
            BOUNCE => match ease {
                EASE_IN => bounce_in(elapsed, begin, change, duration),
                EASE_IN_OUT => bounce_in_out(elapsed, begin, change, duration),
                _ => bounce_out(elapsed, begin, change, duration),
            },
            CIRCULAR => match ease {
                EASE_OUT => circular_out(elapsed, begin, change, duration),
                EASE_IN_OUT => circular_in_out(elapsed, begin, change, duration),
                _ => circular_in(elapsed, begin, change, duration),
            },
            CUBIC => power(elapsed, begin, change, duration, 3, ease),
            ELASTIC => match ease {
                EASE_IN => elastic_in(
                    elapsed,
                    begin,
                    change,
                    duration,
                    previous.amplitude,
                    previous.period,
                ),
                EASE_IN_OUT => elastic_in_out(
                    elapsed,
                    begin,
                    change,
                    duration,
                    previous.amplitude,
                    previous.period,
                ),
                _ => elastic_out(
                    elapsed,
                    begin,
                    change,
                    duration,
                    previous.amplitude,
                    previous.period,
                ),
            },
            EXPONENTIAL => match ease {
                EASE_OUT => exponential_out(elapsed, begin, change, duration),
                EASE_IN_OUT => exponential_in_out(elapsed, begin, change, duration),
                _ => exponential_in(elapsed, begin, change, duration),
            },
            QUADRATIC => power(elapsed, begin, change, duration, 2, ease),
            QUARTIC => power(elapsed, begin, change, duration, 4, ease),
            QUINTIC => power(elapsed, begin, change, duration, 5, ease),
            SINE => match ease {
                EASE_OUT => change * (elapsed / duration * FRAC_PI_2).sin() + begin,
                EASE_IN_OUT => -change / 2.0 * ((PI * elapsed / duration).cos() - 1.0) + begin,
                _ => -change * (elapsed / duration * FRAC_PI_2).cos() + change + begin,
            },
            _ => previous.point.y,
        }
    }
}

fn bezier(previous: Key, key: Key, time: f32) -> f32 {
    let v1 = previous.point;
    let mut v2 = previous.right;
    let mut v3 = key.left;
    let v4 = key.point;
    if (v1.y - v4.y).abs() < f32::EPSILON
        && (v2.y - v3.y).abs() < f32::EPSILON
        && (v3.y - v4.y).abs() < f32::EPSILON
    {
        return v1.y;
    }
    correct_handles(v1, &mut v2, &mut v3, v4);
    let Some(parameter) = find_root(time, v1.x, v2.x, v3.x, v4.x) else {
        return 0.0;
    };
    let c0 = v1.y;
    let c1 = 3.0 * (v2.y - v1.y);
    let c2 = 3.0 * (v1.y - 2.0 * v2.y + v3.y);
    let c3 = v4.y - v1.y + 3.0 * (v2.y - v3.y);
    c0 + parameter * c1 + parameter * parameter * c2 + parameter * parameter * parameter * c3
}

fn correct_handles(v1: Vec2, v2: &mut Vec2, v3: &mut Vec2, v4: Vec2) {
    let h1 = v1 - *v2;
    let h2 = v4 - *v3;
    let span = v4.x - v1.x;
    let len1 = h1.x.abs();
    let len2 = h2.x.abs();
    if len1 + len2 == 0.0 {
        return;
    }
    if len1 > span {
        *v2 = v1 - h1 * (span / len1);
    }
    if len2 > span {
        *v3 = v4 - h2 * (span / len2);
    }
}

fn find_root(x: f32, q0: f32, q1: f32, q2: f32, q3: f32) -> Option<f32> {
    let c0 = f64::from(q0 - x);
    let c1 = f64::from(3.0 * (q1 - q0));
    let c2 = f64::from(3.0 * (q0 - 2.0 * q1 + q2));
    let c3 = f64::from(q3 - q0 + 3.0 * (q1 - q2));
    solve_cubic(c0, c1, c2, c3)
}

fn cube_root(value: f64) -> f64 {
    match value {
        value if value > 0.0 => value.powf(1.0 / 3.0),
        value if value < 0.0 => -(-value).powf(1.0 / 3.0),
        _ => 0.0,
    }
}

fn solve_cubic(c0: f64, c1: f64, c2: f64, c3: f64) -> Option<f32> {
    const SMALL: f32 = -1.0e-10;
    let accept = |root: f64| {
        let root = root as f32;
        (SMALL..=1.000001).contains(&root).then_some(root)
    };
    if c3 != 0.0 {
        let a = c2 / c3 / 3.0;
        let b = c1 / c3;
        let c = c0 / c3;
        let p = b / 3.0 - a * a;
        let q = (2.0 * a * a * a - a * b + c) / 2.0;
        let d = q * q + p * p * p;
        if d > 0.0 {
            let t = d.sqrt();
            return accept(cube_root(-q + t) + cube_root(-q - t) - a);
        }
        if d == 0.0 {
            let t = cube_root(-q);
            return accept(2.0 * t - a).or_else(|| accept(-t - a));
        }
        let phi = (-q / (-(p * p * p)).sqrt()).acos();
        let t = (-p).sqrt();
        let cosine = (phi / 3.0).cos();
        let sine = (3.0 - 3.0 * cosine * cosine).sqrt();
        return accept(2.0 * t * cosine - a)
            .or_else(|| accept(-t * (cosine + sine) - a))
            .or_else(|| accept(-t * (cosine - sine) - a));
    }
    let (a, b, c) = (c2, c1, c0);
    if a != 0.0 {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant > 0.0 {
            let root = discriminant.sqrt();
            return accept((-b - root) / (2.0 * a)).or_else(|| accept((-b + root) / (2.0 * a)));
        }
        if discriminant == 0.0 {
            return accept(-b / (2.0 * a));
        }
        return None;
    }
    if b != 0.0 {
        return accept(-c / b);
    }
    (c == 0.0).then_some(0.0)
}

fn power(time: f32, begin: f32, change: f32, duration: f32, exponent: i32, ease: i64) -> f32 {
    let sign = match exponent % 2 == 0 {
        true => -1.0,
        false => 1.0,
    };
    match ease {
        EASE_OUT => match exponent {
            2 => {
                let time = time / duration;
                -change * time * (time - 2.0) + begin
            }
            _ => {
                let time = time / duration - 1.0;
                sign * change * (time.powi(exponent) + sign) + begin
            }
        },
        EASE_IN_OUT => {
            let time = time / (duration / 2.0);
            if time < 1.0 {
                return change / 2.0 * time.powi(exponent) + begin;
            }
            match exponent {
                2 => {
                    let time = time - 1.0;
                    -change / 2.0 * (time * (time - 2.0) - 1.0) + begin
                }
                _ => {
                    let time = time - 2.0;
                    sign * change / 2.0 * (time.powi(exponent) + 2.0 * sign) + begin
                }
            }
        }
        _ => change * (time / duration).powi(exponent) + begin,
    }
}

fn back_in(time: f32, begin: f32, change: f32, duration: f32, overshoot: f32) -> f32 {
    let time = time / duration;
    change * time * time * ((overshoot + 1.0) * time - overshoot) + begin
}

fn back_out(time: f32, begin: f32, change: f32, duration: f32, overshoot: f32) -> f32 {
    let time = time / duration - 1.0;
    change * (time * time * ((overshoot + 1.0) * time + overshoot) + 1.0) + begin
}

fn back_in_out(time: f32, begin: f32, change: f32, duration: f32, overshoot: f32) -> f32 {
    let overshoot = overshoot * 1.525;
    let time = time / (duration / 2.0);
    if time < 1.0 {
        return change / 2.0 * (time * time * ((overshoot + 1.0) * time - overshoot)) + begin;
    }
    let time = time - 2.0;
    change / 2.0 * (time * time * ((overshoot + 1.0) * time + overshoot) + 2.0) + begin
}

fn bounce_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration;
    if time < 1.0 / 2.75 {
        return change * (7.5625 * time * time) + begin;
    }
    if time < 2.0 / 2.75 {
        let time = time - 1.5 / 2.75;
        return change * (7.5625 * time * time + 0.75) + begin;
    }
    if time < 2.5 / 2.75 {
        let time = time - 2.25 / 2.75;
        return change * (7.5625 * time * time + 0.9375) + begin;
    }
    let time = time - 2.625 / 2.75;
    change * (7.5625 * time * time + 0.984375) + begin
}

fn bounce_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    change - bounce_out(duration - time, 0.0, change, duration) + begin
}

fn bounce_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    if time < duration / 2.0 {
        return bounce_in(time * 2.0, 0.0, change, duration) * 0.5 + begin;
    }
    bounce_out(time * 2.0 - duration, 0.0, change, duration) * 0.5 + change * 0.5 + begin
}

fn circular_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration;
    -change * ((1.0 - time * time).sqrt() - 1.0) + begin
}

fn circular_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration - 1.0;
    change * (1.0 - time * time).sqrt() + begin
}

fn circular_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / (duration / 2.0);
    if time < 1.0 {
        return -change / 2.0 * ((1.0 - time * time).sqrt() - 1.0) + begin;
    }
    let time = time - 2.0;
    change / 2.0 * ((1.0 - time * time).sqrt() + 1.0) + begin
}

const POW_MIN: f32 = 0.000_976_562_5;
const POW_SCALE: f32 = 1.0 / (1.0 - POW_MIN);

fn exponential_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    if time == 0.0 {
        return begin;
    }
    change * (2.0f32.powf(10.0 * (time / duration - 1.0)) - POW_MIN) * POW_SCALE + begin
}

fn exponential_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    if time == 0.0 {
        return begin;
    }
    change * (1.0 - (2.0f32.powf(-10.0 * time / duration) - POW_MIN) * POW_SCALE) + begin
}

fn exponential_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let half = duration / 2.0;
    let change_half = change / 2.0;
    match time <= half {
        true => exponential_in(time, begin, change_half, half),
        false => exponential_out(time - half, begin + change_half, change_half, half),
    }
}

fn elastic_blend(
    time: f32,
    change: f32,
    duration: f32,
    amplitude: f32,
    shift: f32,
    factor: f32,
) -> f32 {
    if change == 0.0 {
        return factor;
    }
    let reach = shift.abs();
    let mut factor = match amplitude != 0.0 {
        true => factor * amplitude / change.abs(),
        false => 0.0,
    };
    if (time * duration).abs() < reach {
        let fraction = (time * duration).abs() / reach;
        factor = factor * fraction + (1.0 - fraction);
    }
    factor
}

fn elastic_shape(
    time: f32,
    change: f32,
    duration: f32,
    amplitude: f32,
    period: f32,
) -> (f32, f32, f32) {
    if amplitude == 0.0 || amplitude < change.abs() {
        let shift = period / 4.0;
        let factor = elastic_blend(time, change, duration, amplitude, shift, 1.0);
        (change, shift, factor)
    } else {
        (
            amplitude,
            period / (2.0 * PI) * (change / amplitude).asin(),
            1.0,
        )
    }
}

fn elastic_in(
    time: f32,
    begin: f32,
    change: f32,
    duration: f32,
    amplitude: f32,
    period: f32,
) -> f32 {
    if time == 0.0 {
        return begin;
    }
    let time = time / duration;
    if time == 1.0 {
        return begin + change;
    }
    let time = time - 1.0;
    let period = match period {
        0.0 => duration * 0.3,
        period => period,
    };
    let (amplitude, shift, factor) = elastic_shape(time, change, duration, amplitude, period);
    -factor
        * (amplitude
            * 2.0f32.powf(10.0 * time)
            * ((time * duration - shift) * (2.0 * PI) / period).sin())
        + begin
}

fn elastic_out(
    time: f32,
    begin: f32,
    change: f32,
    duration: f32,
    amplitude: f32,
    period: f32,
) -> f32 {
    if time == 0.0 {
        return begin;
    }
    let time = time / duration;
    if time == 1.0 {
        return begin + change;
    }
    let time = -time;
    let period = match period {
        0.0 => duration * 0.3,
        period => period,
    };
    let (amplitude, shift, factor) = elastic_shape(time, change, duration, amplitude, period);
    factor
        * (amplitude
            * 2.0f32.powf(10.0 * time)
            * ((time * duration - shift) * (2.0 * PI) / period).sin())
        + change
        + begin
}

fn elastic_in_out(
    time: f32,
    begin: f32,
    change: f32,
    duration: f32,
    amplitude: f32,
    period: f32,
) -> f32 {
    if time == 0.0 {
        return begin;
    }
    let time = time / (duration / 2.0);
    if time == 2.0 {
        return begin + change;
    }
    let time = time - 1.0;
    let period = match period {
        0.0 => duration * (0.3 * 1.5),
        period => period,
    };
    let (amplitude, shift, factor) = elastic_shape(time, change, duration, amplitude, period);
    let wave = amplitude
        * 2.0f32.powf(10.0 * time)
        * ((time * duration - shift) * (2.0 * PI) / period).sin();
    if time < 0.0 {
        return -0.5 * factor * wave + begin;
    }
    let time = -time;
    let wave = amplitude
        * 2.0f32.powf(10.0 * time)
        * ((time * duration - shift) * (2.0 * PI) / period).sin();
    0.5 * factor * wave + change + begin
}
