use std::f32::consts::FRAC_PI_2;

use rikka_shader_graph::{
    Arg, BlendMode, Builder, Constant, Env, GraphKind, NodeKind, Op, SocketRef, SocketValue, Wire,
};

use crate::support::library;

fn computed(build: impl FnOnce(&mut Builder<'_>) -> Wire) -> [f32; 3] {
    let library = library();
    let mut builder = Builder::new(GraphKind::World, &library);
    let value = build(&mut builder);
    let light = builder.external("world.light", [("color", value.into())]);
    let light = builder.out(light, "light");
    let output = builder.node(NodeKind::Output);
    builder.set(output, "lights", light);
    let graph = builder.finish().expect("a valid world");
    let world = graph.world(&library).expect("it plans").evaluate(&Env {
        time: 0.0,
        sockets: &|at: &SocketRef| graph.socket(at).cloned(),
    });
    let color = world.records("lights")[0]
        .number("color")
        .expect("the light has a colour");
    [0, 1, 2].map(|index| color.component(index))
}

fn op(op: Op, inputs: &[(&str, &[f32])], output: &str) -> [f32; 3] {
    computed(|builder| {
        let node = builder.node(NodeKind::Op(op));
        for (port, values) in inputs {
            let constant = Constant::from_slice(values).expect("one to four values");
            builder.set(node, port, Arg::Value(SocketValue::Number(constant)));
        }
        builder.out(node, output)
    })
}

fn close(found: [f32; 3], expected: [f32; 3], tolerance: f32) -> bool {
    found
        .iter()
        .zip(expected)
        .all(|(found, expected)| (found - expected).abs() <= tolerance)
}

#[test]
fn ops_follow_their_formulas() {
    let quarter_turn = FRAC_PI_2;
    let cases: Vec<(&str, [f32; 3], [f32; 3])> = vec![
        (
            "map range linear",
            op(
                Op::MapRange,
                &[
                    ("value", &[0.25]),
                    ("from_high", &[0.5]),
                    ("to_low", &[10.0]),
                    ("to_high", &[20.0]),
                ],
                "value",
            ),
            [15.0; 3],
        ),
        (
            "map range clamps",
            op(
                Op::MapRange,
                &[("value", &[2.0]), ("to_high", &[20.0])],
                "value",
            ),
            [20.0; 3],
        ),
        (
            "map range unclamped",
            op(
                Op::MapRange,
                &[("value", &[2.0]), ("to_high", &[20.0]), ("clamp", &[0.0])],
                "value",
            ),
            [40.0; 3],
        ),
        (
            "map range stepped",
            op(
                Op::MapRange,
                &[("value", &[0.55]), ("mode", &[1.0])],
                "value",
            ),
            [0.5; 3],
        ),
        (
            "map range smoothstep",
            op(
                Op::MapRange,
                &[("value", &[0.25]), ("mode", &[2.0])],
                "value",
            ),
            [0.156_25; 3],
        ),
        (
            "map range smootherstep",
            op(
                Op::MapRange,
                &[("value", &[0.25]), ("mode", &[3.0])],
                "value",
            ),
            [0.103_515_625; 3],
        ),
        (
            "map range reversed smoothstep",
            op(
                Op::MapRange,
                &[
                    ("value", &[0.25]),
                    ("from_low", &[1.0]),
                    ("from_high", &[0.0]),
                    ("mode", &[2.0]),
                ],
                "value",
            ),
            [0.843_75; 3],
        ),
        (
            "map range per component",
            op(
                Op::MapRange,
                &[("value", &[0.0, 0.5, 1.0]), ("to_high", &[2.0])],
                "value",
            ),
            [0.0, 1.0, 2.0],
        ),
        (
            "compare less",
            op(
                Op::Compare,
                &[("a", &[0.3]), ("b", &[0.5]), ("mode", &[0.0])],
                "value",
            ),
            [1.0; 3],
        ),
        (
            "compare equal within epsilon",
            op(
                Op::Compare,
                &[("a", &[0.5]), ("b", &[0.5005]), ("mode", &[4.0])],
                "value",
            ),
            [1.0; 3],
        ),
        (
            "compare not equal",
            op(
                Op::Compare,
                &[("a", &[0.5]), ("b", &[0.5005]), ("mode", &[5.0])],
                "value",
            ),
            [0.0; 3],
        ),
        (
            "switch",
            op(
                Op::Switch,
                &[
                    ("a", &[0.1, 0.2, 0.3]),
                    ("b", &[0.7, 0.8, 0.9]),
                    ("condition", &[1.0]),
                ],
                "value",
            ),
            [0.7, 0.8, 0.9],
        ),
        (
            "smallest component",
            op(Op::ComponentMin, &[("value", &[0.4, 0.1, 0.7])], "value"),
            [0.1; 3],
        ),
        (
            "largest component",
            op(Op::ComponentMax, &[("value", &[0.4, 0.1, 0.7])], "value"),
            [0.7; 3],
        ),
        (
            "radial gradient",
            op(
                Op::Gradient,
                &[("position", &[0.0, 1.0, 0.0]), ("kind", &[6.0])],
                "fac",
            ),
            [0.75; 3],
        ),
        (
            "spherical gradient",
            op(
                Op::Gradient,
                &[("position", &[0.6, 0.0, 0.0]), ("kind", &[4.0])],
                "fac",
            ),
            [0.4; 3],
        ),
        (
            "diagonal gradient",
            op(
                Op::Gradient,
                &[("position", &[0.2, 0.4, 0.0]), ("kind", &[3.0])],
                "fac",
            ),
            [0.3; 3],
        ),
        (
            "checker even cell",
            op(
                Op::Checker,
                &[("position", &[0.5, 0.5, 0.5]), ("first", &[1.0, 0.0, 0.0])],
                "color",
            ),
            [1.0, 0.0, 0.0],
        ),
        (
            "checker odd cell",
            op(
                Op::Checker,
                &[("position", &[1.5, 0.5, 0.5]), ("second", &[0.0, 0.0, 1.0])],
                "color",
            ),
            [0.0, 0.0, 1.0],
        ),
        (
            "mapping point",
            op(
                Op::Mapping,
                &[
                    ("vector", &[1.0, 0.0, 0.0]),
                    ("location", &[0.0, 0.0, 1.0]),
                    ("rotation", &[0.0, 0.0, quarter_turn]),
                ],
                "value",
            ),
            [0.0, 1.0, 1.0],
        ),
        (
            "mapping texture inverts point",
            op(
                Op::Mapping,
                &[
                    ("vector", &[0.0, 1.0, 1.0]),
                    ("location", &[0.0, 0.0, 1.0]),
                    ("rotation", &[0.0, 0.0, quarter_turn]),
                    ("kind", &[1.0]),
                ],
                "value",
            ),
            [1.0, 0.0, 0.0],
        ),
        (
            "rotate about z",
            op(
                Op::VectorRotate,
                &[
                    ("vector", &[1.0, 0.0, 0.0]),
                    ("angle", &[quarter_turn]),
                    ("kind", &[3.0]),
                ],
                "value",
            ),
            [0.0, 1.0, 0.0],
        ),
        (
            "rotate inverted about an axis through a centre",
            op(
                Op::VectorRotate,
                &[
                    ("vector", &[2.0, 1.0, 0.0]),
                    ("center", &[1.0, 1.0, 0.0]),
                    ("axis", &[0.0, 0.0, 2.0]),
                    ("angle", &[quarter_turn]),
                    ("invert", &[1.0]),
                ],
                "value",
            ),
            [1.0, 0.0, 0.0],
        ),
        (
            "hue turned a third",
            op(
                Op::HueSaturation,
                &[("color", &[1.0, 0.0, 0.0]), ("hue", &[0.5 + 1.0 / 3.0])],
                "value",
            ),
            [0.0, 1.0, 0.0],
        ),
        (
            "value scaled",
            op(
                Op::HueSaturation,
                &[("color", &[1.0, 0.0, 0.0]), ("value", &[0.5])],
                "value",
            ),
            [0.5, 0.0, 0.0],
        ),
        (
            "brightness contrast",
            op(
                Op::BrightnessContrast,
                &[
                    ("color", &[0.5, 0.5, 0.5]),
                    ("brightness", &[0.1]),
                    ("contrast", &[0.5]),
                ],
                "value",
            ),
            [0.6; 3],
        ),
        (
            "gamma",
            op(
                Op::Gamma,
                &[("color", &[0.25, 0.25, 0.25]), ("gamma", &[0.5])],
                "value",
            ),
            [0.5; 3],
        ),
        (
            "invert",
            op(Op::Invert, &[("color", &[0.2, 0.2, 0.2])], "value"),
            [0.8; 3],
        ),
        (
            "linear light blend",
            computed(|builder| builder.blend(BlendMode::LinearLight, [0.5; 3], [0.75; 3], 1.0)),
            [1.0; 3],
        ),
        (
            "exclusion blend",
            computed(|builder| builder.blend(BlendMode::Exclusion, [0.5; 3], [0.5; 3], 1.0)),
            [0.5; 3],
        ),
        (
            "divide blend",
            computed(|builder| builder.blend(BlendMode::Divide, [0.5; 3], [0.25, 0.0, 0.5], 1.0)),
            [2.0, 0.0, 1.0],
        ),
        (
            "hue blend",
            computed(|builder| {
                builder.blend(BlendMode::Hue, [1.0, 0.0, 0.0], [0.0, 0.0, 0.5], 1.0)
            }),
            [0.0, 0.0, 1.0],
        ),
        (
            "value blend",
            computed(|builder| builder.blend(BlendMode::Value, [1.0, 0.0, 0.0], [0.25; 3], 1.0)),
            [0.25, 0.0, 0.0],
        ),
    ];
    for (name, found, expected) in cases {
        assert!(
            close(found, expected, 1e-4),
            "{name}: {found:?} against {expected:?}"
        );
    }
}

#[test]
fn light_colours_follow_physics() {
    let luminance = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let white = op(Op::Blackbody, &[("temperature", &[6500.0])], "value");
    assert!((luminance(white) - 1.0).abs() < 0.02, "{white:?}");
    assert!(
        white.iter().all(|channel| (channel - 1.0).abs() < 0.15),
        "{white:?}"
    );
    let candle = op(Op::Blackbody, &[("temperature", &[1900.0])], "value");
    assert!(candle[0] > candle[1] && candle[1] > candle[2], "{candle:?}");
    for (nanometres, strongest) in [(450.0, 2), (530.0, 1), (610.0, 0)] {
        let color = op(Op::Wavelength, &[("wavelength", &[nanometres])], "value");
        let found = (0..3)
            .max_by(|a, b| color[*a].total_cmp(&color[*b]))
            .expect("three channels");
        assert_eq!(found, strongest, "{nanometres} nm gives {color:?}");
    }
}

#[test]
fn white_noise_is_repeatable_and_seeded() {
    let at = |seed: f32| {
        op(
            Op::WhiteNoise,
            &[("position", &[1.5, 2.25, 0.75]), ("seed", &[seed])],
            "color",
        )
    };
    assert_eq!(at(1.0), at(1.0));
    assert_ne!(at(1.0), at(2.0));
    assert!(at(3.0).iter().all(|value| (0.0..=1.0).contains(value)));
}
