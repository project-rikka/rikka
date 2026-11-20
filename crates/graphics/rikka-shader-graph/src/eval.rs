use std::collections::BTreeMap;

use crate::emit::sample;
use crate::flat::{Flat, FlatNode, Slot, Source};
use crate::graph::SocketRef;
use crate::node::{BlendMode, Input, Interpolation, NodeKind, Noise};
use crate::ops::Op;
use crate::ports::swizzle;
use crate::types::{Constant, SocketValue, ValueType};

type Point = [f32; 3];

pub(crate) fn pcg3_bits(mut v: [u32; 3]) -> [u32; 3] {
    for value in &mut v {
        *value = value.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    }
    let mix = |v: &mut [u32; 3]| {
        v[0] = v[0].wrapping_add(v[1].wrapping_mul(v[2]));
        v[1] = v[1].wrapping_add(v[2].wrapping_mul(v[0]));
        v[2] = v[2].wrapping_add(v[0].wrapping_mul(v[1]));
    };
    mix(&mut v);
    for value in &mut v {
        *value ^= *value >> 16;
    }
    mix(&mut v);
    v
}

pub(crate) fn hash3(cell: Point) -> Point {
    pcg3_bits(cell.map(|value| (value as i32) as u32))
        .map(|value| value as f32 * (1.0 / 4_294_967_295.0))
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn corners(i: Point, f: Point, u: Point, corner: &dyn Fn(Point, Point, Point) -> f32) -> f32 {
    let at = |x: f32, y: f32, z: f32| corner(i, f, [x, y, z]);
    let x00 = lerp(at(0.0, 0.0, 0.0), at(1.0, 0.0, 0.0), u[0]);
    let x10 = lerp(at(0.0, 1.0, 0.0), at(1.0, 1.0, 0.0), u[0]);
    let x01 = lerp(at(0.0, 0.0, 1.0), at(1.0, 0.0, 1.0), u[0]);
    let x11 = lerp(at(0.0, 1.0, 1.0), at(1.0, 1.0, 1.0), u[0]);
    lerp(lerp(x00, x10, u[1]), lerp(x01, x11, u[1]), u[2])
}

fn split(p: Point) -> (Point, Point) {
    let i = p.map(f32::floor);
    (i, [p[0] - i[0], p[1] - i[1], p[2] - i[2]])
}

fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn value_noise(p: Point) -> f32 {
    let (i, f) = split(p);
    let u = f.map(|f| f * f * (3.0 - 2.0 * f));
    corners(i, f, u, &|i, _, corner| hash3(add(i, corner))[0])
}

fn gradient_noise(p: Point) -> f32 {
    let (i, f) = split(p);
    let u = f.map(|f| f * f * f * (f * (f * 6.0 - 15.0) + 10.0));
    let corner = |i: Point, f: Point, corner: Point| {
        let raw = hash3(add(i, corner)).map(|value| value * 2.0 - 1.0 + 1e-6);
        let length = (raw[0] * raw[0] + raw[1] * raw[1] + raw[2] * raw[2]).sqrt();
        (0..3)
            .map(|axis| raw[axis] / length * (f[axis] - corner[axis]))
            .sum()
    };
    corners(i, f, u, &corner) * 0.5 + 0.5
}

fn voronoi(p: Point, randomness: f32) -> [f32; 2] {
    let (i, f) = split(p);
    let mut nearest = 8.0;
    let mut cell = 0.0;
    for z in -1..=1 {
        for y in -1..=1 {
            for x in -1..=1 {
                let offset = [x as f32, y as f32, z as f32];
                let jitter = hash3(add(i, offset));
                let gap = (0..3)
                    .map(|axis| {
                        let side = offset[axis] + jitter[axis] * randomness - f[axis];
                        side * side
                    })
                    .sum::<f32>()
                    .sqrt();
                if gap < nearest {
                    nearest = gap;
                    cell = jitter[1];
                }
            }
        }
    }
    [nearest, cell]
}

pub(crate) fn fbm(noise: Noise, mut p: Point, octaves: f32, lacunarity: f32, gain: f32) -> f32 {
    let count = (octaves as i32).clamp(1, 8);
    let mut sum = 0.0;
    let mut amplitude = 1.0;
    let mut total = 0.0;
    for _ in 0..count {
        let value = match noise {
            Noise::Value => value_noise(p),
            Noise::Gradient => gradient_noise(p),
        };
        sum += value * amplitude;
        total += amplitude;
        p = p.map(|axis| axis * lacunarity);
        amplitude *= gain;
    }
    sum / f32::max(total, 1e-6)
}

pub(crate) fn point(value: &Constant, scale: f32) -> Point {
    let at = |index: usize| match index < value.width() {
        true => value.component(index) * scale,
        false => 0.0,
    };
    [at(0), at(1), at(2)]
}

fn rgb(value: &Constant) -> Point {
    [value.component(0), value.component(1), value.component(2)]
}

fn luma(c: Point) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

pub(crate) fn rgb_to_hsv(c: Point) -> Point {
    let (p, k) = match c[1] < c[2] {
        true => ([c[2], c[1]], [-1.0, 2.0 / 3.0]),
        false => ([c[1], c[2]], [0.0, -1.0 / 3.0]),
    };
    let p = [p[0], p[1], k[0], k[1]];
    let q = match c[0] < p[0] {
        true => [p[0], p[1], p[3], c[0]],
        false => [c[0], p[1], p[2], p[0]],
    };
    let d = q[0] - q[3].min(q[1]);
    [
        (q[2] + (q[3] - q[1]) / (6.0 * d + 1e-10)).abs(),
        d / (q[0] + 1e-10),
        q[0],
    ]
}

pub(crate) fn hsv_to_rgb(c: Point) -> Point {
    [1.0, 2.0 / 3.0, 1.0 / 3.0].map(|offset| {
        let p = ((c[0] + offset).rem_euclid(1.0) * 6.0 - 3.0).abs();
        c[2] * lerp(1.0, (p - 1.0).clamp(0.0, 1.0), c[1])
    })
}

fn vec3(value: Point) -> Constant {
    Constant::vec3(value[0], value[1], value[2])
}

pub(crate) fn compute(op: Op, args: &[Constant], width: usize) -> Option<Vec<Constant>> {
    let dot = |a: &Constant, b: &Constant| -> f32 {
        (0..width).map(|i| a.component(i) * b.component(i)).sum()
    };
    let each = |f: &dyn Fn(usize) -> f32| {
        let values: Vec<f32> = (0..width).map(f).collect();
        vec![Constant::from_slice(&values).expect("a computed width")]
    };
    Some(match op {
        Op::Reflect => {
            let along = 2.0 * dot(&args[1], &args[0]);
            each(&|i| args[0].component(i) - along * args[1].component(i))
        }
        Op::Refract => {
            let eta = args[2].component(0);
            let cosine = dot(&args[1], &args[0]);
            let k = 1.0 - eta * eta * (1.0 - cosine * cosine);
            match k < 0.0 {
                true => each(&|_| 0.0),
                false => each(&|i| {
                    eta * args[0].component(i) - (eta * cosine + k.sqrt()) * args[1].component(i)
                }),
            }
        }
        Op::Rotate => {
            let [uv, center] = [0, 1].map(|at| [args[at].component(0), args[at].component(1)]);
            let (sine, cosine) = args[2].component(0).sin_cos();
            let d = [uv[0] - center[0], uv[1] - center[1]];
            vec![Constant::vec2(
                center[0] + cosine * d[0] - sine * d[1],
                center[1] + sine * d[0] + cosine * d[1],
            )]
        }
        Op::Polar => {
            let [uv, center] = [0, 1].map(|at| [args[at].component(0), args[at].component(1)]);
            let d = [uv[0] - center[0], uv[1] - center[1]];
            vec![Constant::vec2(
                d[0].hypot(d[1]),
                d[1].atan2(d[0]) / std::f32::consts::TAU + 0.5,
            )]
        }
        Op::ValueNoise => vec![Constant::scalar(value_noise(point(
            &args[0],
            args[1].component(0),
        )))],
        Op::GradientNoise => vec![Constant::scalar(gradient_noise(point(
            &args[0],
            args[1].component(0),
        )))],
        Op::Voronoi => {
            let [distance, cell] =
                voronoi(point(&args[0], args[1].component(0)), args[2].component(0));
            vec![Constant::scalar(distance), Constant::scalar(cell)]
        }
        Op::Fresnel => {
            let facing: f32 = (0..3)
                .map(|i| args[0].component(i) * args[1].component(i))
                .sum();
            vec![Constant::scalar(
                (1.0 - facing.clamp(0.0, 1.0)).powf(args[2].component(0)),
            )]
        }
        Op::RgbToHsv => vec![vec3(rgb_to_hsv(rgb(&args[0])))],
        Op::HsvToRgb => vec![vec3(hsv_to_rgb(rgb(&args[0])))],
        Op::Saturation => {
            let color = rgb(&args[0]);
            let grey = luma(color);
            vec![vec3(
                color.map(|channel| lerp(grey, channel, args[1].component(0))),
            )]
        }
        _ => return None,
    })
}

fn blend(mode: BlendMode, base: Point, top: Point, opacity: f32) -> Point {
    let each = |f: &dyn Fn(f32, f32) -> f32| [0, 1, 2].map(|i| f(base[i], top[i]));
    let mixed = match mode {
        BlendMode::Normal => top,
        BlendMode::Multiply => each(&|a, b| a * b),
        BlendMode::Screen => each(&|a, b| 1.0 - (1.0 - a) * (1.0 - b)),
        BlendMode::Overlay => each(&|a, b| match a >= 0.5 {
            true => 1.0 - 2.0 * (1.0 - a) * (1.0 - b),
            false => 2.0 * a * b,
        }),
        BlendMode::SoftLight => each(&|a, b| match b >= 0.5 {
            true => a + (2.0 * b - 1.0) * (a.sqrt() - a),
            false => a - (1.0 - 2.0 * b) * a * (1.0 - a),
        }),
        BlendMode::Add => each(&|a, b| a + b),
        BlendMode::Subtract => each(&|a, b| a - b),
        BlendMode::Difference => each(&|a, b| (a - b).abs()),
        BlendMode::Darken => each(&|a, b| a.min(b)),
        BlendMode::Lighten => each(&|a, b| a.max(b)),
        BlendMode::Dodge => each(&|a, b| (a / (1.0 - b).max(1e-5)).min(1.0)),
        BlendMode::Burn => each(&|a, b| 1.0 - ((1.0 - a) / b.max(1e-5)).min(1.0)),
        BlendMode::Divide => each(&|a, b| if b != 0.0 { a / b } else { 0.0 }),
        BlendMode::LinearLight => each(&|a, b| a + 2.0 * b - 1.0),
        BlendMode::Exclusion => each(&|a, b| a + b - 2.0 * a * b),
        BlendMode::Hue => {
            let above = rgb_to_hsv(top);
            match above[1] == 0.0 {
                true => base,
                false => {
                    let below = rgb_to_hsv(base);
                    hsv_to_rgb([above[0], below[1], below[2]])
                }
            }
        }
        BlendMode::Saturation => {
            let below = rgb_to_hsv(base);
            match below[1] == 0.0 {
                true => base,
                false => hsv_to_rgb([below[0], rgb_to_hsv(top)[1], below[2]]),
            }
        }
        BlendMode::Color => {
            let above = rgb_to_hsv(top);
            match above[1] == 0.0 {
                true => base,
                false => hsv_to_rgb([above[0], above[1], rgb_to_hsv(base)[2]]),
            }
        }
        BlendMode::Value => {
            let below = rgb_to_hsv(base);
            hsv_to_rgb([below[0], below[1], rgb_to_hsv(top)[2]])
        }
    };
    [0, 1, 2].map(|i| lerp(base[i], mixed[i], opacity))
}

#[derive(Clone, Debug, PartialEq)]
enum Step {
    Constant(Constant),
    Socket {
        at: SocketRef,
        width: usize,
        fallback: Constant,
    },
    Time,
    Resize {
        from: usize,
        width: usize,
    },
    Op {
        op: Op,
        args: Vec<usize>,
        width: usize,
        pick: usize,
    },
    Swizzle {
        from: usize,
        picks: Vec<usize>,
    },
    Blend {
        mode: BlendMode,
        args: [usize; 3],
    },
    Piecewise {
        at: usize,
        points: Vec<(f32, Constant)>,
        interpolation: Interpolation,
        ty: ValueType,
    },
    Fbm {
        noise: Noise,
        args: [usize; 5],
    },
}

pub struct Env<'a> {
    pub time: f32,
    pub sockets: &'a dyn Fn(&SocketRef) -> Option<SocketValue>,
}

impl std::fmt::Debug for Env<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Env")
            .field("time", &self.time)
            .finish_non_exhaustive()
    }
}

impl Step {
    fn args(&self) -> &[usize] {
        match self {
            Self::Resize { from, .. } | Self::Swizzle { from, .. } => std::slice::from_ref(from),
            Self::Piecewise { at, .. } => std::slice::from_ref(at),
            Self::Op { args, .. } => args,
            Self::Blend { args, .. } => args,
            Self::Fbm { args, .. } => args,
            Self::Constant(_) | Self::Socket { .. } | Self::Time => &[],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reads {
    pub time: bool,
    pub sockets: Vec<SocketRef>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    steps: Vec<Step>,
}

impl Program {
    pub fn run(&self, env: &Env<'_>, values: &mut Vec<Constant>) {
        self.run_until(env, values, self.steps.len());
    }

    pub fn reads(&self, roots: impl IntoIterator<Item = usize>) -> Reads {
        let mut seen = vec![false; self.steps.len()];
        let mut pending: Vec<usize> = roots
            .into_iter()
            .filter(|root| *root < self.steps.len())
            .collect();
        let mut reads = Reads::default();
        while let Some(at) = pending.pop() {
            if std::mem::replace(&mut seen[at], true) {
                continue;
            }
            match &self.steps[at] {
                Step::Time => reads.time = true,
                Step::Socket { at, .. } => {
                    if !reads.sockets.contains(at) {
                        reads.sockets.push(at.clone());
                    }
                }
                step => pending.extend_from_slice(step.args()),
            }
        }
        reads
    }

    pub fn run_until(&self, env: &Env<'_>, values: &mut Vec<Constant>, end: usize) {
        values.clear();
        for step in &self.steps[..end.min(self.steps.len())] {
            let value = match step {
                Step::Constant(constant) => *constant,
                Step::Socket {
                    at,
                    width,
                    fallback,
                } => (env.sockets)(at)
                    .and_then(|held| held.number())
                    .unwrap_or(*fallback)
                    .resized(*width),
                Step::Time => Constant::scalar(env.time),
                Step::Resize { from, width } => values[*from].resized(*width),
                Step::Op {
                    op,
                    args,
                    width,
                    pick,
                } => {
                    let args: Vec<Constant> = args.iter().map(|arg| values[*arg]).collect();
                    op.compute(&args, *width)
                        .and_then(|outputs| outputs.get(*pick).copied())
                        .unwrap_or(Constant::scalar(0.0))
                }
                Step::Swizzle { from, picks } => {
                    let picked: Vec<f32> = picks
                        .iter()
                        .map(|index| values[*from].component(*index))
                        .collect();
                    Constant::from_slice(&picked).expect("a swizzle")
                }
                Step::Blend { mode, args } => vec3(blend(
                    *mode,
                    rgb(&values[args[0]]),
                    rgb(&values[args[1]]),
                    values[args[2]].component(0),
                )),
                Step::Piecewise {
                    at,
                    points,
                    interpolation,
                    ty,
                } => sample(points, values[*at].component(0), *interpolation, *ty),
                Step::Fbm { noise, args } => {
                    let [position, scale, octaves, lacunarity, gain] = args.map(|arg| values[arg]);
                    Constant::scalar(fbm(
                        *noise,
                        point(&position, scale.component(0)),
                        octaves.component(0),
                        lacunarity.component(0),
                        gain.component(0),
                    ))
                }
            };
            values.push(value);
        }
    }
}

pub(crate) struct Compiler<'f, 'g> {
    flat: &'f Flat<'g>,
    steps: Vec<Step>,
    known: BTreeMap<Slot, usize>,
}

impl<'f, 'g> Compiler<'f, 'g> {
    pub(crate) fn new(flat: &'f Flat<'g>) -> Self {
        Self {
            flat,
            steps: Vec::new(),
            known: BTreeMap::new(),
        }
    }

    pub(crate) fn finish(self) -> Program {
        Program { steps: self.steps }
    }

    fn push(&mut self, step: Step) -> usize {
        self.steps.push(step);
        self.steps.len() - 1
    }

    pub(crate) fn input(&mut self, node: &FlatNode<'_>, port: &str) -> usize {
        let types = self.flat.types(node);
        let target = match node.kind {
            NodeKind::FunctionInput(_) => types.outputs.get(port),
            _ => types.inputs.get(port),
        }
        .copied()
        .expect("a checked input has a type");
        let width = target.width().unwrap_or(1);
        match node.input(port) {
            Source::Link(from, output) => {
                let from_width = self
                    .flat
                    .output_type(from, output)
                    .and_then(ValueType::width)
                    .unwrap_or(width);
                let step = self.value(&(from.clone(), output.clone()));
                match from_width == width {
                    true => step,
                    false => self.push(Step::Resize { from: step, width }),
                }
            }
            Source::Constant(constant) => self.push(Step::Constant(constant.resized(width))),
            Source::Socket(at, held) => self.push(Step::Socket {
                at: at.clone(),
                width,
                fallback: held.number().unwrap_or(Constant::scalar(0.0)),
            }),
            Source::Context(Input::Time) => self.push(Step::Time),
            _ => self.push(Step::Constant(Constant::scalar(0.0).resized(width))),
        }
    }

    fn value(&mut self, slot: &Slot) -> usize {
        if let Some(step) = self.known.get(slot) {
            return *step;
        }
        let node = &self.flat.nodes[&slot.0];
        let step = match node.kind {
            NodeKind::Constant(constant) => self.push(Step::Constant(*constant)),
            NodeKind::Input(Input::Time) => self.push(Step::Time),
            NodeKind::FunctionInput(_) | NodeKind::FunctionOutput(_) => self.input(node, "value"),
            NodeKind::Op(op) => {
                let info = op.info();
                let args: Vec<usize> = info
                    .inputs
                    .iter()
                    .map(|port| self.input(node, port.name))
                    .collect();
                let width = self.flat.types(node).inputs[info.inputs[0].name]
                    .width()
                    .expect("ops read vectors");
                let pick = match op.packed() {
                    Some((_, parts)) => parts
                        .iter()
                        .position(|(port, _)| *port == slot.1)
                        .unwrap_or(0),
                    None => 0,
                };
                self.push(Step::Op {
                    op: *op,
                    args,
                    width,
                    pick,
                })
            }
            NodeKind::Swizzle(mask) => {
                let from = self.input(node, "value");
                self.push(Step::Swizzle {
                    from,
                    picks: swizzle(mask).expect("a checked swizzle"),
                })
            }
            NodeKind::Blend(mode) => {
                let args = ["base", "blend", "opacity"].map(|port| self.input(node, port));
                self.push(Step::Blend { mode: *mode, args })
            }
            NodeKind::ColorRamp(ramp) => {
                let at = self.input(node, "position");
                let mut stops = ramp.stops.clone();
                stops.sort_by(|a, b| a.position.total_cmp(&b.position));
                self.push(Step::Piecewise {
                    at,
                    points: stops
                        .iter()
                        .map(|stop| {
                            let [r, g, b, a] = stop.color;
                            (stop.position, Constant::vec4(r, g, b, a))
                        })
                        .collect(),
                    interpolation: ramp.interpolation,
                    ty: ValueType::Float4,
                })
            }
            NodeKind::Curve(curve) => {
                let at = self.input(node, "value");
                let mut sorted = curve.points.clone();
                sorted.sort_by(|a, b| a[0].total_cmp(&b[0]));
                self.push(Step::Piecewise {
                    at,
                    points: sorted
                        .iter()
                        .map(|[x, y]| (*x, Constant::scalar(*y)))
                        .collect(),
                    interpolation: curve.interpolation,
                    ty: ValueType::Float,
                })
            }
            NodeKind::Fbm(noise) => {
                let args = ["position", "scale", "octaves", "lacunarity", "gain"]
                    .map(|port| self.input(node, port));
                self.push(Step::Fbm {
                    noise: *noise,
                    args,
                })
            }
            _ => self.push(Step::Constant(Constant::scalar(0.0))),
        };
        self.known.insert(slot.clone(), step);
        step
    }
}
