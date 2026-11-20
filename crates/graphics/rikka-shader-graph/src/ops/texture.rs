use crate::eval::{fbm, hash3, pcg3_bits, point};
use crate::node::{Noise, PortSpec, out, value};
use crate::ops::{Category, Op, OpInfo, POSITION};
use crate::types::{Constant, PortType, ValueType};

const SCALE: PortSpec = value("scale", ValueType::Float, Constant::scalar(1.0));
const TAU: f32 = std::f32::consts::TAU;

pub(crate) fn info(op: Op) -> OpInfo {
    let fac = [out("fac", PortType::Value(ValueType::Float))];
    let colored = [
        out("color", PortType::Value(ValueType::Float3)),
        out("fac", PortType::Value(ValueType::Float)),
    ];
    match op {
        Op::WhiteNoise => OpInfo::new(
            "White noise",
            Category::Noise,
            "A random value and colour in 0…1 for every distinct position and seed, without smoothing.",
            &[POSITION, PortSpec::number("seed", 0.0)],
            &[
                out("value", PortType::Value(ValueType::Float)),
                out("color", PortType::Value(ValueType::Float3)),
            ],
        ),
        Op::Checker => OpInfo::new(
            "Checker texture",
            Category::Texture,
            "Alternating cubes of size 1/scale: fac 1 and first where floor(x)+floor(y)+floor(z) is even, else 0 and second.",
            &[
                POSITION,
                SCALE,
                PortSpec::color("first", 0.8, 0.8, 0.8),
                PortSpec::color("second", 0.2, 0.2, 0.2),
            ],
            &colored,
        ),
        Op::Brick => OpInfo::new(
            "Brick texture",
            Category::Texture,
            "Blender's brick pattern in the xy plane: bricks tinted randomly between first and second (bias shifts the tint), fac 1 in the mortar.",
            &[
                POSITION,
                SCALE,
                PortSpec::color("first", 0.8, 0.8, 0.8),
                PortSpec::color("second", 0.2, 0.2, 0.2),
                PortSpec::color("mortar", 0.0, 0.0, 0.0),
                PortSpec::number("mortar_size", 0.02).soft(0.0, 0.125),
                PortSpec::number("mortar_smooth", 0.1).range(0.0, 1.0),
                PortSpec::number("bias", 0.0).range(-1.0, 1.0),
                PortSpec::number("brick_width", 0.5).soft(0.01, 2.0),
                PortSpec::number("row_height", 0.25).soft(0.01, 2.0),
                PortSpec::number("offset", 0.5).range(0.0, 1.0),
                PortSpec::number("offset_frequency", 2.0).soft(1.0, 8.0),
                PortSpec::number("squash", 1.0).soft(0.0, 4.0),
                PortSpec::number("squash_frequency", 2.0).soft(1.0, 8.0),
            ],
            &colored,
        ),
        Op::Wave => OpInfo::new(
            "Wave texture",
            Category::Texture,
            "Blender's wave texture: bands or rings 20 per unit along direction, shaped as sine, saw or triangle, shifted by phase and wobbled by gradient-noise distortion.",
            &[
                POSITION,
                SCALE,
                PortSpec::number("distortion", 0.0).soft(-10.0, 10.0),
                PortSpec::number("detail", 2.0).range(0.0, 7.0),
                PortSpec::number("detail_scale", 1.0).soft(0.0, 10.0),
                PortSpec::number("roughness", 0.5).range(0.0, 1.0),
                PortSpec::number("phase", 0.0).hint(crate::node::Hint::Angle),
                PortSpec::options("kind", &["bands", "rings"], 0),
                PortSpec::options("direction", &["x", "y", "z", "diagonal"], 0),
                PortSpec::options("profile", &["sine", "saw", "triangle"], 0),
            ],
            &fac,
        ),
        Op::Gradient => OpInfo::new(
            "Gradient texture",
            Category::Texture,
            "Blender's gradient of position: linear x, quadratic max(x,0)², easing, diagonal (x+y)/2, spherical 1-|p|, quadratic sphere or radial angle/2π+0.5.",
            &[
                POSITION,
                PortSpec::options(
                    "kind",
                    &[
                        "linear",
                        "quadratic",
                        "easing",
                        "diagonal",
                        "spherical",
                        "quadratic_sphere",
                        "radial",
                    ],
                    0,
                ),
            ],
            &fac,
        ),
        _ => unreachable!("{op:?} is not a texture op"),
    }
}

pub(crate) fn expression(op: Op, args: &[String]) -> String {
    let a = |index: usize| args[index].as_str();
    match op {
        Op::WhiteNoise => format!("rikka_graph_white_noise({}, {})", a(0), a(1)),
        Op::Checker => format!(
            "rikka_graph_checker({} * {}, {}, {})",
            a(0),
            a(1),
            a(2),
            a(3)
        ),
        Op::Brick => format!(
            "rikka_graph_brick({} * {}, {}, {}, {}, rikka_graph_brick_shape({}))",
            a(0),
            a(1),
            a(2),
            a(3),
            a(4),
            args[5..].join(", ")
        ),
        Op::Wave => format!(
            "rikka_graph_wave({} * {}, {})",
            a(0),
            a(1),
            args[2..].join(", ")
        ),
        Op::Gradient => format!("rikka_graph_gradient({}, {})", a(0), a(1)),
        _ => unreachable!("{op:?} is not a texture op"),
    }
}

pub(crate) fn packed(op: Op) -> Option<(ValueType, &'static [(&'static str, &'static str)])> {
    match op {
        Op::WhiteNoise => Some((ValueType::Float4, &[("value", ".w"), ("color", ".xyz")])),
        Op::Checker | Op::Brick => Some((ValueType::Float4, &[("color", ".xyz"), ("fac", ".w")])),
        _ => None,
    }
}

fn vec3(value: [f32; 3]) -> Constant {
    Constant::vec3(value[0], value[1], value[2])
}

fn rgb(value: &Constant) -> [f32; 3] {
    [value.component(0), value.component(1), value.component(2)]
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn smooth(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn white_noise(p: [f32; 3], seed: f32) -> [f32; 3] {
    let seed = seed.to_bits();
    let salts = [1u32, 0x9e37_79b9, 0x85eb_ca6b];
    let bits = [0, 1, 2].map(|axis| p[axis].to_bits() ^ seed.wrapping_mul(salts[axis]));
    pcg3_bits(bits).map(|value| value as f32 * (1.0 / 4_294_967_295.0))
}

fn checker(p: [f32; 3]) -> f32 {
    let sum = p
        .iter()
        .map(|axis| axis.floor() as i32)
        .fold(0i32, i32::wrapping_add);
    if sum & 1 == 0 { 1.0 } else { 0.0 }
}

fn brick(p: [f32; 3], v: &[f32]) -> (f32, f32) {
    let [
        mortar_size,
        mortar_smooth,
        bias,
        width,
        height,
        offset,
        offset_frequency,
        squash,
        squash_frequency,
    ] = [v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8]];
    let height = height.max(1e-5);
    let row = (p[1] / height).floor();
    let mut width = width.max(1e-5);
    let offset_frequency = (offset_frequency as i32).max(1);
    let squash_frequency = (squash_frequency as i32).max(1);
    let rows = row as i32;
    if rows % squash_frequency == 0 {
        width *= squash.max(1e-5);
    }
    let shift = match rows % offset_frequency == 0 {
        true => width * offset,
        false => 0.0,
    };
    let column = ((p[0] + shift) / width).floor();
    let x = p[0] + shift - width * column;
    let y = p[1] - height * row;
    let tint = (hash3([row, column, 0.0])[0] + bias).clamp(0.0, 1.0);
    let edge = x.min(y).min((width - x).min(height - y));
    let mortar = match (edge < mortar_size, mortar_smooth <= 0.0) {
        (false, _) => 0.0,
        (true, true) => 1.0,
        (true, false) => smooth(0.0, mortar_smooth, 1.0 - edge / mortar_size.max(1e-5)),
    };
    (tint, mortar)
}

fn wave(p: [f32; 3], v: &[f32]) -> f32 {
    let [
        distortion,
        detail,
        detail_scale,
        roughness,
        phase,
        kind,
        direction,
        profile,
    ] = [v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]];
    let axis = direction as i32;
    let mut n = match kind as i32 {
        0 => match axis {
            0 => p[0] * 20.0,
            1 => p[1] * 20.0,
            2 => p[2] * 20.0,
            _ => (p[0] + p[1] + p[2]) * 10.0,
        },
        _ => {
            let ring = match axis {
                0 => [0.0, p[1], p[2]],
                1 => [p[0], 0.0, p[2]],
                2 => [p[0], p[1], 0.0],
                _ => p,
            };
            (ring[0] * ring[0] + ring[1] * ring[1] + ring[2] * ring[2]).sqrt() * 20.0
        }
    };
    n += phase;
    if distortion != 0.0 {
        let scaled = p.map(|axis| axis * detail_scale);
        n += distortion * (fbm(Noise::Gradient, scaled, detail + 1.0, 2.0, roughness) * 2.0 - 1.0);
    }
    match profile as i32 {
        1 => {
            let turns = n / TAU;
            turns - turns.floor()
        }
        2 => {
            let turns = n / TAU;
            (turns - (turns + 0.5).floor()).abs() * 2.0
        }
        _ => 0.5 + 0.5 * (n - std::f32::consts::FRAC_PI_2).sin(),
    }
}

fn gradient(p: [f32; 3], kind: f32) -> f32 {
    let length = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    match kind as i32 {
        1 => p[0].max(0.0).powi(2),
        2 => {
            let r = p[0].clamp(0.0, 1.0);
            r * r * (3.0 - 2.0 * r)
        }
        3 => (p[0] + p[1]) * 0.5,
        4 => (1.0 - length).max(0.0),
        5 => (1.0 - length).max(0.0).powi(2),
        6 => p[1].atan2(p[0]) / TAU + 0.5,
        _ => p[0],
    }
}

pub(crate) fn compute(op: Op, args: &[Constant]) -> Vec<Constant> {
    let scalars: Vec<f32> = args.iter().map(|arg| arg.component(0)).collect();
    match op {
        Op::WhiteNoise => {
            let hashed = white_noise(point(&args[0], 1.0), scalars[1]);
            vec![Constant::scalar(hashed[0]), vec3(hashed)]
        }
        Op::Checker => {
            let fac = checker(point(&args[0], scalars[1]));
            vec![
                vec3(mix(rgb(&args[3]), rgb(&args[2]), fac)),
                Constant::scalar(fac),
            ]
        }
        Op::Brick => {
            let (tint, mortar) = brick(point(&args[0], scalars[1]), &scalars[5..]);
            let color = mix(
                mix(rgb(&args[2]), rgb(&args[3]), tint),
                rgb(&args[4]),
                mortar,
            );
            vec![vec3(color), Constant::scalar(mortar)]
        }
        Op::Wave => vec![Constant::scalar(wave(
            point(&args[0], scalars[1]),
            &scalars[2..],
        ))],
        Op::Gradient => vec![Constant::scalar(gradient(point(&args[0], 1.0), scalars[1]))],
        _ => unreachable!("{op:?} is not a texture op"),
    }
}
