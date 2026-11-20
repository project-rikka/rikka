use crate::document::Node;

const DEFAULT_WEIGHT: f32 = 0.333_333;

const CONSTANT: u32 = 0x2;
const CUBIC: u32 = 0x8;
const TANGENT_TCB: u32 = 0x200;
const TANGENT_USER: u32 = 0x400;
const CONSTANT_NEXT: u32 = 0x100;
const CLAMP: u32 = 0x1000;
const TIME_INDEPENDENT: u32 = 0x2000;
const CLAMP_PROGRESSIVE: u32 = 0x4000;
const WEIGHTED_RIGHT: u32 = 0x0100_0000;
const WEIGHTED_NEXT_LEFT: u32 = 0x0200_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    HoldPrevious,
    HoldNext,
    Linear,
    Cubic,
}

#[derive(Clone, Copy, Debug, Default)]
struct Tangent {
    dx: f32,
    dy: f32,
}

#[derive(Clone, Copy, Debug)]
struct Key {
    time: f64,
    value: f32,
    shape: Shape,
    left: Tangent,
    right: Tangent,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Curve {
    keys: Vec<Key>,
}

impl Curve {
    pub(crate) fn read(node: &Node, ktime: i64) -> Self {
        let times = node.child("KeyTime").map(Node::longs);
        let values = node.child("KeyValueFloat").map(Node::floats);
        let (Some(times), Some(values)) = (times, values) else {
            return Self::default();
        };
        let flags = node
            .child("KeyAttrFlags")
            .map(Node::ints)
            .unwrap_or_default();
        let data = node
            .child("KeyAttrDataFloat")
            .map(Node::floats)
            .unwrap_or_default();
        let repeats = node
            .child("KeyAttrRefCount")
            .map(Node::ints)
            .unwrap_or_default();

        let count = times.len().min(values.len());
        let mut runs = Runs::new(&flags, &data, &repeats, count);
        let mut solver = Solver::new(count);
        for index in 0..count {
            let (flags, attributes) = runs.next();
            solver.push(
                times[index] as f64 / ktime as f64,
                values[index],
                flags,
                attributes,
                |offset| {
                    let neighbour_index = (index as isize + offset) as usize;
                    Some((times.get(neighbour_index)?, values.get(neighbour_index)?))
                        .map(|(time, value)| (*time as f64 / ktime as f64, *value))
                },
            );
        }
        Self {
            keys: solver.finish(),
        }
    }

    pub(crate) fn read_legacy(node: &Node, ktime: i64, key_version: i64) -> Self {
        let Some(keys) = node.child("Key") else {
            return Self::default();
        };
        let data = keys.doubles();
        let count = node
            .child_integer("KeyCount")
            .and_then(|count| usize::try_from(count).ok())
            .unwrap_or(0);

        let mut read = Legacy {
            data: &data,
            at: 0,
            key_version,
        };
        let mut legacy_keys: Vec<(f64, f32, Shape, f32, f32)> = Vec::with_capacity(count);
        for _ in 0..count {
            match read.key(ktime) {
                Some(key) => legacy_keys.push(key),
                None => break,
            }
        }

        let mut solver = Solver::new(legacy_keys.len());
        for (index, &(time, value, shape, slope_right, slope_left)) in
            legacy_keys.iter().enumerate()
        {
            solver.push_legacy(time, value, shape, (slope_right, slope_left), |offset| {
                let neighbour = legacy_keys.get((index as isize + offset) as usize)?;
                Some((neighbour.0, neighbour.1))
            });
        }
        Self {
            keys: solver.finish(),
        }
    }

    pub(crate) fn times(&self) -> impl Iterator<Item = f64> + '_ {
        self.keys.iter().map(|key| key.time)
    }

    pub(crate) fn evaluate(&self, time: f64, fallback: f32) -> f32 {
        let [first, .., last] = self.keys.as_slice() else {
            return self.keys.first().map_or(fallback, |key| key.value);
        };
        if time <= first.time {
            return first.value;
        }
        if time >= last.time {
            return last.value;
        }
        let end = self.keys.partition_point(|key| key.time <= time);
        let (previous, next) = (&self.keys[end - 1], &self.keys[end]);
        let width = next.time - previous.time;
        if width <= 0.0 {
            return previous.value;
        }
        let progress = (time - previous.time) / width;

        match previous.shape {
            Shape::HoldPrevious => previous.value,
            Shape::HoldNext => next.value,
            Shape::Linear => {
                (f64::from(previous.value) * (1.0 - progress) + f64::from(next.value) * progress)
                    as f32
            }
            Shape::Cubic => {
                let x1 = f64::from(previous.right.dx) / width;
                let x2 = 1.0 - f64::from(next.left.dx) / width;
                let param = bezier_at(x1, x2, progress);
                let (squared, cubed) = (param * param, param * param * param);
                let rest = 1.0 - param;
                let (rest2, rest3) = (rest * rest, rest * rest * rest);
                let y0 = f64::from(previous.value);
                let y3 = f64::from(next.value);
                let y1 = y0 + f64::from(previous.right.dy);
                let y2 = y3 - f64::from(next.left.dy);
                (rest3 * y0 + 3.0 * (rest2 * param * y1 + rest * squared * y2) + cubed * y3) as f32
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Slope {
    value: f32,
    weight: f32,
}

impl Default for Slope {
    fn default() -> Self {
        Self {
            value: 0.0,
            weight: DEFAULT_WEIGHT,
        }
    }
}

impl Slope {
    fn new(value: f32) -> Self {
        Self {
            value,
            ..Self::default()
        }
    }

    fn tangent(self, width: f64) -> Tangent {
        let dx = self.weight * width as f32;
        Tangent {
            dx,
            dy: dx * self.value,
        }
    }
}

struct Solved {
    time: f64,
    value: f32,
    shape: Shape,
    right: Slope,
    next_left: Slope,
}

struct Solver {
    keys: Vec<Key>,
    left: Slope,
}

impl Solver {
    fn new(count: usize) -> Self {
        Self {
            keys: Vec::with_capacity(count),
            left: Slope::default(),
        }
    }

    fn push(
        &mut self,
        time: f64,
        value: f32,
        flags: u32,
        attributes: [f32; 4],
        neighbour: impl Fn(isize) -> Option<(f64, f32)>,
    ) {
        let mut right = Slope::new(attributes[0]);
        let mut next_left = Slope::new(attributes[1]);

        if flags & (WEIGHTED_RIGHT | WEIGHTED_NEXT_LEFT) != 0 {
            let packed = attributes[2].to_bits();
            if flags & WEIGHTED_RIGHT != 0 {
                right.weight = (packed & 0xffff) as f32 * 0.0001;
            }
            if flags & WEIGHTED_NEXT_LEFT != 0 {
                next_left.weight = (packed >> 16) as f32 * 0.0001;
            }
        }

        let previous = neighbour(-1);
        let next = neighbour(1);
        let shape = if flags & CONSTANT != 0 {
            right = Slope::default();
            next_left = Slope::default();
            match flags & CONSTANT_NEXT != 0 {
                true => Shape::HoldNext,
                false => Shape::HoldPrevious,
            }
        } else if flags & CUBIC != 0 {
            if flags & TANGENT_TCB != 0 {
                let (slope_left, slope_right) = tcb(
                    f64::from(attributes[0]),
                    f64::from(attributes[1]),
                    f64::from(attributes[2]),
                    previous
                        .filter(|(previous_time, _)| time > *previous_time)
                        .map(|(previous_time, previous_value)| {
                            slope(previous_time, previous_value, time, value)
                        }),
                    next.filter(|(next_time, _)| *next_time > time)
                        .map(|(next_time, next_value)| slope(time, value, next_time, next_value)),
                );
                self.left.value = slope_left;
                right.value = slope_right;
                next_left = Slope::default();
            } else if flags & TANGENT_USER == 0 {
                let (slope_left, slope_right) =
                    automatic((time, value), previous, next, self.left, right, flags);
                self.left.value = slope_left;
                right.value = slope_right;
            }
            Shape::Cubic
        } else {
            let linear_slope = straight(time, value, next);
            right = Slope::new(linear_slope);
            next_left = Slope::new(linear_slope);
            Shape::Linear
        };

        self.emit(
            Solved {
                time,
                value,
                shape,
                right,
                next_left,
            },
            previous,
            next,
        );
    }

    fn push_legacy(
        &mut self,
        time: f64,
        value: f32,
        shape: Shape,
        slopes: (f32, f32),
        neighbour: impl Fn(isize) -> Option<(f64, f32)>,
    ) {
        let previous = neighbour(-1);
        let next = neighbour(1);
        let (right, next_left) = match shape {
            Shape::Linear => {
                let linear_slope = straight(time, value, next);
                (linear_slope, linear_slope)
            }
            Shape::Cubic => slopes,
            _ => (0.0, 0.0),
        };
        self.emit(
            Solved {
                time,
                value,
                shape,
                right: Slope::new(right),
                next_left: Slope::new(next_left),
            },
            previous,
            next,
        );
    }

    fn emit(&mut self, solved: Solved, previous: Option<(f64, f32)>, next: Option<(f64, f32)>) {
        let left = match previous
            .map(|(previous_time, _)| previous_time)
            .filter(|previous_time| solved.time > *previous_time)
        {
            Some(previous_time) => self.left.tangent(solved.time - previous_time),
            None => Tangent::default(),
        };
        let right = match next
            .map(|(next_time, _)| next_time)
            .filter(|next_time| *next_time > solved.time)
        {
            Some(next_time) => solved.right.tangent(next_time - solved.time),
            None => Tangent::default(),
        };
        self.keys.push(Key {
            time: solved.time,
            value: solved.value,
            shape: solved.shape,
            left,
            right,
        });
        self.left = solved.next_left;
    }

    fn finish(self) -> Vec<Key> {
        self.keys
    }
}

fn straight(time: f64, value: f32, next: Option<(f64, f32)>) -> f32 {
    next.filter(|(next_time, _)| *next_time > time)
        .map_or(0.0, |(next_time, next_value)| {
            slope(time, value, next_time, next_value) as f32
        })
}

struct Runs<'a> {
    flags: &'a [i32],
    data: &'a [f32],
    repeats: &'a [i32],
    run: usize,
    left: i32,
}

impl<'a> Runs<'a> {
    fn new(flags: &'a [i32], data: &'a [f32], repeats: &'a [i32], count: usize) -> Self {
        Self {
            flags,
            data,
            repeats,
            run: 0,
            left: repeats.first().copied().unwrap_or(count as i32),
        }
    }

    fn next(&mut self) -> (u32, [f32; 4]) {
        if self.left <= 0 && self.run + 1 < self.repeats.len() {
            self.run += 1;
            self.left = self.repeats[self.run];
        }
        self.left -= 1;
        let flags = self.flags.get(self.run).copied().unwrap_or(0) as u32;
        let first = self.run * 4;
        let mut attributes = [0.0; 4];
        for (slot, attribute) in attributes.iter_mut().enumerate() {
            *attribute = self.data.get(first + slot).copied().unwrap_or(0.0);
        }
        (flags, attributes)
    }
}

struct Legacy<'a> {
    data: &'a [f64],
    at: usize,
    key_version: i64,
}

impl Legacy<'_> {
    fn key(&mut self, ktime: i64) -> Option<(f64, f32, Shape, f32, f32)> {
        let time = self.take()? / ktime as f64;
        let value = self.take()? as f32;
        let mode = letter(self.take()?);

        let mut slope_right = 0.0;
        let mut next_slope_left = 0.0;
        let (shape, weights) = match mode {
            b'C' => {
                let next = letter(self.peek().unwrap_or(0.0));
                let shape = match next == b'n' {
                    true => {
                        self.at += 1;
                        Shape::HoldNext
                    }
                    false => Shape::HoldPrevious,
                };
                (shape, 0)
            }
            b'U' => {
                let slope_mode = letter(self.take()?);
                let weights = match slope_mode {
                    b's' | b'b' => {
                        slope_right = self.take()? as f32;
                        next_slope_left = self.take()? as f32;
                        match self.key_version == 4003 {
                            true => 0,
                            false => 1,
                        }
                    }
                    b'a' => match self.key_version <= 4004 {
                        true => 0,
                        false => 1,
                    },
                    b'p' | b'q' => {
                        self.at += 2;
                        match self.key_version <= 4004 {
                            true => 1,
                            false => 2,
                        }
                    }
                    b't' => {
                        self.at += 3;
                        0
                    }
                    _ => 1,
                };
                (Shape::Cubic, weights)
            }
            _ => (Shape::Linear, 0),
        };
        self.at += weights;
        Some((time, value, shape, slope_right, next_slope_left))
    }

    fn take(&mut self) -> Option<f64> {
        let value = self.data.get(self.at).copied()?;
        self.at += 1;
        Some(value)
    }

    fn peek(&self) -> Option<f64> {
        self.data.get(self.at).copied()
    }
}

fn letter(value: f64) -> u8 {
    match (0.0..256.0).contains(&value) {
        true => value as u8,
        false => 0,
    }
}

fn slope(from_time: f64, from: f32, to_time: f64, to: f32) -> f64 {
    match to_time > from_time {
        true => (f64::from(to) - f64::from(from)) / (to_time - from_time),
        false => 0.0,
    }
}

fn tcb(
    tension: f64,
    continuity: f64,
    bias: f64,
    left: Option<f64>,
    right: Option<f64>,
) -> (f32, f32) {
    let edge = left.is_none() || right.is_none();
    let factor = match edge {
        true => 1.0,
        false => 0.5,
    };
    let (left, right) = (left.unwrap_or(0.0), right.unwrap_or(0.0));
    let d00 = factor * (1.0 - tension) * (1.0 + bias) * (1.0 - continuity);
    let d01 = factor * (1.0 - tension) * (1.0 - bias) * (1.0 + continuity);
    let d10 = factor * (1.0 - tension) * (1.0 + bias) * (1.0 + continuity);
    let d11 = factor * (1.0 - tension) * (1.0 - bias) * (1.0 - continuity);
    (
        (d00 * left + d01 * right) as f32,
        (d10 * left + d11 * right) as f32,
    )
}

fn automatic(
    key: (f64, f32),
    previous: Option<(f64, f32)>,
    next: Option<(f64, f32)>,
    left: Slope,
    right: Slope,
    flags: u32,
) -> (f32, f32) {
    let (time, value) = key;
    match (previous, next) {
        (Some(previous), Some(next)) if next.0 > time && time > previous.0 => {
            let solve =
                |bias: f32| between(previous, key, next, left.weight, right.weight, bias, flags);
            match (left.value + right.value).abs() <= 0.0001 {
                true => {
                    let shared_slope = solve(right.value);
                    (shared_slope, shared_slope)
                }
                false => (solve(-left.value), solve(right.value)),
            }
        }
        (Some(previous), _) if time > previous.0 => {
            let edge_slope = edge(
                slope(previous.0, previous.1, time, value),
                -left.value,
                flags,
            );
            (edge_slope, edge_slope)
        }
        (_, Some(next)) if next.0 > time => {
            let edge_slope = edge(slope(time, value, next.0, next.1), right.value, flags);
            (edge_slope, edge_slope)
        }
        _ => (0.0, 0.0),
    }
}

fn between(
    previous: (f64, f32),
    key: (f64, f32),
    next: (f64, f32),
    weight_left: f32,
    weight_right: f32,
    bias: f32,
    flags: u32,
) -> f32 {
    let (previous_time, previous_value) = previous;
    let (time, value) = key;
    let (next_time, next_value) = next;

    if flags & CLAMP != 0 {
        let nearest = (f64::from(previous_value) - f64::from(value))
            .abs()
            .min((f64::from(next_value) - f64::from(value)).abs());
        if nearest <= 1e-9 {
            return 0.0;
        }
    }

    let mut slope =
        (f64::from(next_value) - f64::from(previous_value)) / (next_time - previous_time);
    if flags & TIME_INDEPENDENT == 0 {
        let left = (f64::from(value) - f64::from(previous_value)) / (time - previous_time);
        let right = (f64::from(next_value) - f64::from(value)) / (next_time - time);
        let along = (time - previous_time) / (next_time - previous_time);
        slope = slope * 0.5 + (left * (1.0 - along) + right * along) * 0.5;
        slope = biased(slope, left, right, f64::from(bias));
    }

    if flags & CLAMP_PROGRESSIVE != 0 {
        let sign = match slope >= 0.0 {
            true => 1.0,
            false => -1.0,
        };
        let mut magnitude = sign * slope;
        let range_left = f64::from(weight_left) * (time - previous_time);
        let range_right = f64::from(weight_right) * (next_time - time);
        let limit = |range: f64, difference: f64| match range > 0.0 {
            true => (sign * difference / range).max(0.0),
            false => 0.0,
        };
        magnitude = magnitude.min(limit(
            range_left,
            f64::from(value) - f64::from(previous_value),
        ));
        magnitude = magnitude.min(limit(range_right, f64::from(next_value) - f64::from(value)));
        slope = sign * magnitude;
    }
    slope as f32
}

fn biased(slope: f64, left: f64, right: f64, bias: f64) -> f64 {
    let weight = bias.abs() / 100.0;
    if weight <= 0.0001 {
        return slope;
    }
    let target = match bias > 0.0 {
        true => right,
        false => left,
    };
    let difference = target - slope;
    let mut biased_slope = slope * (1.0 - weight) + target * weight;
    let beyond = weight - 5.0;
    if beyond > 0.0 {
        let sign = match difference.abs() > 0.00001 {
            true => difference,
            false => bias,
        };
        let sign = match sign > 0.0 {
            true => 1.0,
            false => -1.0,
        };
        biased_slope += beyond * beyond * sign * 40.0;
    }
    biased_slope
}

fn edge(slope: f64, bias: f32, flags: u32) -> f32 {
    if flags & CLAMP_PROGRESSIVE != 0 {
        return 0.0;
    }
    let mut biased_slope = slope;
    if flags & TIME_INDEPENDENT == 0 {
        let beyond = f64::from(bias).abs() / 100.0 - 5.0;
        if beyond > 0.0 {
            let sign = match bias > 0.0 {
                true => 1.0,
                false => -1.0,
            };
            biased_slope += beyond * beyond * sign * 40.0;
        }
    }
    biased_slope as f32
}

fn bezier_at(x1: f64, x2: f64, target: f64) -> f64 {
    let a = 3.0 * x1 - 3.0 * x2 + 1.0;
    let b = 3.0 * x2 - 6.0 * x1;
    let c = 3.0 * x1;
    let mut param = target;
    for _ in 0..8 {
        let squared = param * param;
        let error = a * squared * param + b * squared + c * param - target;
        if error.abs() <= 1e-12 {
            break;
        }
        let derivative = 3.0 * a * squared + 2.0 * b * param + c;
        if derivative.abs() < 1e-12 {
            break;
        }
        param -= error / derivative;
    }
    param.clamp(0.0, 1.0)
}
