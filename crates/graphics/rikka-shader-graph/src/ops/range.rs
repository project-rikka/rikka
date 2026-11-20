use crate::node::{PortSpec, dynamic, out};
use crate::ops::{Category, Op, OpInfo};
use crate::types::{Constant, PortType, ValueType};

const MAP_MODES: &[&str] = &["linear", "stepped", "smoothstep", "smootherstep"];
const COMPARE_MODES: &[&str] = &[
    "less",
    "less_equal",
    "greater",
    "greater_equal",
    "equal",
    "not_equal",
];

pub(crate) fn info(op: Op) -> OpInfo {
    let result = [out("value", PortType::Dynamic)];
    let scalar = [out("value", PortType::Value(ValueType::Float))];
    match op {
        Op::MapRange => OpInfo::new(
            "Map range",
            Category::Interpolation,
            "Maps value from from_low…from_high onto to_low…to_high: linear, in steps+1 equal stairs, or eased; clamp keeps it inside the target range.",
            &[
                dynamic("value", 0.5),
                dynamic("from_low", 0.0),
                dynamic("from_high", 1.0),
                dynamic("to_low", 0.0),
                dynamic("to_high", 1.0),
                PortSpec::number("steps", 4.0).soft(1.0, 16.0),
                PortSpec::toggle("clamp", true),
                PortSpec::options("mode", MAP_MODES, 0),
            ],
            &result,
        ),
        Op::Compare => OpInfo::new(
            "Compare",
            Category::Math,
            "1 when a relates to b as mode says, else 0; equal and not equal allow a difference up to epsilon.",
            &[
                PortSpec::number("a", 0.0),
                PortSpec::number("b", 0.5),
                PortSpec::number("epsilon", 0.001).soft(0.0, 0.1),
                PortSpec::options("mode", COMPARE_MODES, 2),
            ],
            &scalar,
        ),
        Op::Switch => OpInfo::new(
            "Switch",
            Category::Math,
            "b when condition is above 0.5, else a.",
            &[
                dynamic("a", 0.0),
                dynamic("b", 1.0),
                PortSpec::toggle("condition", false),
            ],
            &result,
        ),
        Op::ComponentMin => OpInfo::new(
            "Smallest component",
            Category::Vector,
            "The smallest component of value.",
            &[dynamic("value", 0.0)],
            &scalar,
        ),
        Op::ComponentMax => OpInfo::new(
            "Largest component",
            Category::Vector,
            "The largest component of value.",
            &[dynamic("value", 0.0)],
            &scalar,
        ),
        _ => unreachable!("{op:?} is not a range op"),
    }
}

fn componentwise(name: &str, args: &[String], lanes: usize, width: usize) -> String {
    let call = |swizzle: &str| {
        let parts: Vec<String> = args
            .iter()
            .enumerate()
            .map(|(index, arg)| match index < lanes && width > 1 {
                true => format!("{arg}{swizzle}"),
                false => arg.clone(),
            })
            .collect();
        format!("{name}({})", parts.join(", "))
    };
    match width {
        1 => call(""),
        _ => {
            let lanes: Vec<String> = [".x", ".y", ".z", ".w"][..width]
                .iter()
                .map(|swizzle| call(swizzle))
                .collect();
            format!("float{width}({})", lanes.join(", "))
        }
    }
}

pub(crate) fn expression(op: Op, args: &[String], ty: ValueType) -> String {
    match op {
        Op::MapRange => componentwise(
            "rikka_graph_map_range",
            args,
            5,
            ty.width().expect("a vector"),
        ),
        Op::Compare => format!("rikka_graph_compare({})", args.join(", ")),
        Op::Switch => format!("({} > 0.5 ? {} : {})", args[2], args[1], args[0]),
        Op::ComponentMin => format!("rikka_graph_min_component({})", args[0]),
        Op::ComponentMax => format!("rikka_graph_max_component({})", args[0]),
        _ => unreachable!("{op:?} is not a range op"),
    }
}

fn smooth(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smoother(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub(crate) fn map_range(v: &[f32]) -> f32 {
    let [
        value,
        from_low,
        from_high,
        to_low,
        to_high,
        steps,
        clamped,
        mode,
    ] = [v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]];
    let kind = mode as i32;
    let mut t = 0.0;
    if from_low != from_high {
        let eased = |ease: fn(f32, f32, f32) -> f32| match from_low > from_high {
            true => 1.0 - ease(from_high, from_low, value),
            false => ease(from_low, from_high, value),
        };
        t = match kind {
            2 => eased(smooth),
            3 => eased(smoother),
            _ => {
                let linear = (value - from_low) / (from_high - from_low);
                match (kind, steps > 0.0) {
                    (1, true) => (linear * (steps + 1.0)).floor() / steps,
                    (1, false) => 0.0,
                    _ => linear,
                }
            }
        };
    }
    let result = to_low + t * (to_high - to_low);
    match clamped > 0.5 {
        true => result.clamp(to_low.min(to_high), to_low.max(to_high)),
        false => result,
    }
}

fn compare(a: f32, b: f32, epsilon: f32, mode: f32) -> f32 {
    let holds = match mode as i32 {
        0 => a < b,
        1 => a <= b,
        2 => a > b,
        3 => a >= b,
        4 => (a - b).abs() <= epsilon,
        _ => (a - b).abs() > epsilon,
    };
    if holds { 1.0 } else { 0.0 }
}

pub(crate) fn compute(op: Op, args: &[Constant], width: usize) -> Vec<Constant> {
    let each = |f: &dyn Fn(&[f32]) -> f32| {
        let values: Vec<f32> = (0..width)
            .map(|index| {
                let at: Vec<f32> = args.iter().map(|arg| arg.component(index)).collect();
                f(&at)
            })
            .collect();
        vec![Constant::from_slice(&values).expect("a computed width")]
    };
    let scalar = |value: f32| vec![Constant::scalar(value)];
    let components = || args[0].values().iter().copied();
    match op {
        Op::MapRange => each(&map_range),
        Op::Compare => scalar(compare(
            args[0].component(0),
            args[1].component(0),
            args[2].component(0),
            args[3].component(0),
        )),
        Op::Switch => match args[2].component(0) > 0.5 {
            true => vec![args[1].resized(width)],
            false => vec![args[0].resized(width)],
        },
        Op::ComponentMin => scalar(components().fold(f32::INFINITY, f32::min)),
        Op::ComponentMax => scalar(components().fold(f32::NEG_INFINITY, f32::max)),
        _ => unreachable!("{op:?} is not a range op"),
    }
}
