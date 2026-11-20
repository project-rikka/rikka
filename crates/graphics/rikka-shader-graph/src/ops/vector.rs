use crate::node::{Hint, PortSpec, out};
use crate::ops::{Category, Op, OpInfo};
use crate::types::{Constant, PortType, ValueType};

type Vec3 = [f32; 3];
type Mat3 = [Vec3; 3];

pub(crate) fn info(op: Op) -> OpInfo {
    let spatial = [out("value", PortType::Value(ValueType::Float3))];
    let rotation = PortSpec::vector("rotation", 0.0, 0.0, 0.0).hint(Hint::Angle);
    match op {
        Op::Mapping => OpInfo::new(
            "Mapping",
            Category::Vector,
            "Blender's mapping with XYZ Euler rotation in radians: point R(v·S)+T, texture its inverse S⁻¹Rᵀ(v-T), vector R(v·S), normal normalize(R(v/S)).",
            &[
                PortSpec::vector("vector", 0.0, 0.0, 0.0),
                PortSpec::vector("location", 0.0, 0.0, 0.0),
                rotation,
                PortSpec::vector("scale", 1.0, 1.0, 1.0),
                PortSpec::options("kind", &["point", "texture", "vector", "normal"], 0),
            ],
            &spatial,
        ),
        Op::VectorRotate => OpInfo::new(
            "Vector rotate",
            Category::Vector,
            "vector turned about center: by angle radians around axis or a principal axis, or by an XYZ Euler rotation; invert turns the other way.",
            &[
                PortSpec::vector("vector", 0.0, 0.0, 0.0),
                PortSpec::vector("center", 0.0, 0.0, 0.0),
                PortSpec::vector("axis", 0.0, 0.0, 1.0),
                PortSpec::number("angle", 0.0).hint(Hint::Angle),
                rotation,
                PortSpec::options("kind", &["axis_angle", "x", "y", "z", "euler"], 0),
                PortSpec::toggle("invert", false),
            ],
            &spatial,
        ),
        _ => unreachable!("{op:?} is not a vector op"),
    }
}

pub(crate) fn expression(op: Op, args: &[String]) -> String {
    match op {
        Op::Mapping => format!("rikka_graph_mapping({})", args.join(", ")),
        Op::VectorRotate => format!("rikka_graph_vector_rotate({})", args.join(", ")),
        _ => unreachable!("{op:?} is not a vector op"),
    }
}

fn euler(angles: Vec3) -> Mat3 {
    let [cx, cy, cz] = angles.map(f32::cos);
    let [sx, sy, sz] = angles.map(f32::sin);
    [
        [cy * cz, cy * sz, -sy],
        [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy],
        [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy],
    ]
}

fn apply(columns: &Mat3, v: Vec3) -> Vec3 {
    [0, 1, 2].map(|row| columns[0][row] * v[0] + columns[1][row] * v[1] + columns[2][row] * v[2])
}

fn apply_transposed(columns: &Mat3, v: Vec3) -> Vec3 {
    columns.map(|column| column[0] * v[0] + column[1] * v[1] + column[2] * v[2])
}

fn safe_divide(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| if b[i] != 0.0 { a[i] / b[i] } else { 0.0 })
}

fn normalized(v: Vec3) -> Vec3 {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    match length > 0.0 {
        true => v.map(|axis| axis / length),
        false => v,
    }
}

fn mapping(v: Vec3, location: Vec3, rotation: Vec3, scale: Vec3, kind: f32) -> Vec3 {
    let turn = euler(rotation);
    let scaled = [0, 1, 2].map(|i| v[i] * scale[i]);
    match kind as i32 {
        1 => {
            let moved = [0, 1, 2].map(|i| v[i] - location[i]);
            let inverse = [0, 1, 2].map(|i| if scale[i] != 0.0 { 1.0 / scale[i] } else { 0.0 });
            let turned = apply_transposed(&turn, moved);
            [0, 1, 2].map(|i| turned[i] * inverse[i])
        }
        2 => apply(&turn, scaled),
        3 => normalized(apply(&turn, safe_divide(v, scale))),
        _ => {
            let turned = apply(&turn, scaled);
            [0, 1, 2].map(|i| turned[i] + location[i])
        }
    }
}

fn axis_angle(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if length <= 0.0 {
        return v;
    }
    let k = axis.map(|component| component / length);
    let (s, c) = angle.sin_cos();
    let along = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
    let cross = [
        k[1] * v[2] - k[2] * v[1],
        k[2] * v[0] - k[0] * v[2],
        k[0] * v[1] - k[1] * v[0],
    ];
    [0, 1, 2].map(|i| v[i] * c + cross[i] * s + k[i] * along * (1.0 - c))
}

fn rotate(
    v: Vec3,
    center: Vec3,
    axis: Vec3,
    angle: f32,
    rotation: Vec3,
    kind: f32,
    invert: f32,
) -> Vec3 {
    let sign = if invert > 0.5 { -1.0 } else { 1.0 };
    let d = [0, 1, 2].map(|i| v[i] - center[i]);
    let turned = match kind as i32 {
        1 => axis_angle(d, [1.0, 0.0, 0.0], angle * sign),
        2 => axis_angle(d, [0.0, 1.0, 0.0], angle * sign),
        3 => axis_angle(d, [0.0, 0.0, 1.0], angle * sign),
        4 => {
            let turn = euler(rotation);
            match invert > 0.5 {
                true => apply_transposed(&turn, d),
                false => apply(&turn, d),
            }
        }
        _ => axis_angle(d, axis, angle * sign),
    };
    [0, 1, 2].map(|i| turned[i] + center[i])
}

pub(crate) fn compute(op: Op, args: &[Constant]) -> Vec<Constant> {
    let v = |index: usize| -> Vec3 { [0, 1, 2].map(|i| args[index].component(i)) };
    let s = |index: usize| args[index].component(0);
    let result = match op {
        Op::Mapping => mapping(v(0), v(1), v(2), v(3), s(4)),
        Op::VectorRotate => rotate(v(0), v(1), v(2), s(3), v(4), s(5), s(6)),
        _ => unreachable!("{op:?} is not a vector op"),
    };
    vec![Constant::vec3(result[0], result[1], result[2])]
}
