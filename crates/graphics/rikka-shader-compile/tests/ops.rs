use rikka_renderer_3d::{Draw, MaterialDesc};
use rikka_shader_graph::{
    Arg, Behavior, Builder, Category, Constant, Env, GraphKind, Hosts, Input, Library, NodeKind,
    NodeSpec, Op, PortSpec, SocketRef, SocketValue, Symbol, ValueType, Wire,
};

use crate::support::{
    SCREEN_HALF, SIZE, compiled, compiler, encoded, frame, material, near, pixels, quad, renderer,
    rgb,
};

const PROBE: ValueType = ValueType::Record(Symbol::fixed("probe"));
const COLUMNS: usize = 5;
const ROWS: usize = 4;

struct Case {
    op: Op,
    output: &'static str,
    inputs: &'static [(&'static str, &'static [f32])],
    scale: f32,
    bias: f32,
}

const CASES: &[Case] = &[
    Case {
        op: Op::MapRange,
        output: "value",
        inputs: &[
            ("value", &[0.3]),
            ("to_low", &[0.2]),
            ("to_high", &[0.8]),
            ("mode", &[2.0]),
        ],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::MapRange,
        output: "value",
        inputs: &[
            ("value", &[0.55]),
            ("to_low", &[0.2]),
            ("to_high", &[0.8]),
            ("mode", &[1.0]),
        ],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::Compare,
        output: "value",
        inputs: &[("a", &[0.3]), ("b", &[0.5]), ("mode", &[0.0])],
        scale: 0.7,
        bias: 0.0,
    },
    Case {
        op: Op::Switch,
        output: "value",
        inputs: &[("a", &[0.2]), ("b", &[0.9]), ("condition", &[1.0])],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::ComponentMax,
        output: "value",
        inputs: &[("value", &[0.2, 0.6, 0.4])],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::WhiteNoise,
        output: "color",
        inputs: &[("position", &[1.5, 2.25, 0.75]), ("seed", &[3.0])],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::Checker,
        output: "color",
        inputs: &[
            ("position", &[0.5, 1.5, 0.25]),
            ("first", &[0.9, 0.1, 0.1]),
            ("second", &[0.1, 0.1, 0.9]),
        ],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::Brick,
        output: "color",
        inputs: &[("position", &[0.3, 0.1, 0.0]), ("first", &[0.9, 0.5, 0.2])],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::Wave,
        output: "fac",
        inputs: &[("position", &[0.13, 0.2, 0.3]), ("distortion", &[1.5])],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::Gradient,
        output: "fac",
        inputs: &[("position", &[0.3, 0.4, 0.0]), ("kind", &[2.0])],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::Mapping,
        output: "value",
        inputs: &[
            ("vector", &[0.5, 0.2, 0.1]),
            ("location", &[0.1, 0.2, 0.3]),
            ("rotation", &[0.3, 0.2, 0.5]),
            ("scale", &[0.5, 1.0, 1.5]),
        ],
        scale: 0.5,
        bias: 0.4,
    },
    Case {
        op: Op::Mapping,
        output: "value",
        inputs: &[
            ("vector", &[0.5, 0.2, 0.1]),
            ("location", &[0.1, 0.2, 0.3]),
            ("rotation", &[0.3, 0.2, 0.5]),
            ("scale", &[0.5, 1.0, 1.5]),
            ("kind", &[1.0]),
        ],
        scale: 0.5,
        bias: 0.4,
    },
    Case {
        op: Op::VectorRotate,
        output: "value",
        inputs: &[
            ("vector", &[0.6, 0.1, 0.3]),
            ("rotation", &[0.4, 0.9, 0.2]),
            ("kind", &[4.0]),
        ],
        scale: 0.5,
        bias: 0.4,
    },
    Case {
        op: Op::VectorRotate,
        output: "value",
        inputs: &[
            ("vector", &[0.6, 0.1, 0.3]),
            ("axis", &[1.0, 1.0, 0.0]),
            ("angle", &[0.7]),
        ],
        scale: 0.5,
        bias: 0.4,
    },
    Case {
        op: Op::HueSaturation,
        output: "value",
        inputs: &[
            ("color", &[0.8, 0.3, 0.2]),
            ("hue", &[0.7]),
            ("saturation", &[0.8]),
            ("value", &[0.9]),
        ],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::BrightnessContrast,
        output: "value",
        inputs: &[
            ("color", &[0.4, 0.5, 0.6]),
            ("brightness", &[0.1]),
            ("contrast", &[0.3]),
        ],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::Gamma,
        output: "value",
        inputs: &[("color", &[0.4, 0.5, 0.6]), ("gamma", &[2.2])],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::Invert,
        output: "value",
        inputs: &[("color", &[0.2, 0.5, 0.7]), ("factor", &[0.8])],
        scale: 1.0,
        bias: 0.0,
    },
    Case {
        op: Op::Blackbody,
        output: "value",
        inputs: &[("temperature", &[4000.0])],
        scale: 0.5,
        bias: 0.0,
    },
    Case {
        op: Op::Wavelength,
        output: "value",
        inputs: &[("wavelength", &[550.0])],
        scale: 0.3,
        bias: 0.0,
    },
];

fn library() -> Library {
    Library::standard()
        .with(NodeSpec {
            id: "probe",
            title: "Probe",
            category: Category::Light,
            doc: "",
            hosts: Hosts::WORLD,
            inputs: vec![PortSpec::vector("value", 0.0, 0.0, 0.0)],
            outputs: vec![PortSpec::output("probe", PROBE)],
            behavior: Behavior::Record,
        })
        .with_world(vec![PortSpec::empty("probe", PROBE)])
}

fn op(builder: &mut Builder<'_>, case: &Case) -> Wire {
    let node = builder.node(NodeKind::Op(case.op));
    for (port, values) in case.inputs {
        let constant = Constant::from_slice(values).expect("one to four values");
        builder.set(node, port, Arg::Value(SocketValue::Number(constant)));
    }
    builder.out(node, case.output)
}

fn on_cpu(library: &Library, case: &Case) -> [f32; 3] {
    let mut builder = Builder::new(GraphKind::World, library);
    let value = op(&mut builder, case);
    let probe = builder.external("probe", [("value", value.into())]);
    let probe = builder.out(probe, "probe");
    builder.output([("probe", probe.into())]);
    let graph = builder.finish().expect("a valid world");
    let world = graph.world(library).expect("it plans").evaluate(&Env {
        time: 0.0,
        sockets: &|at: &SocketRef| graph.socket(at).cloned(),
    });
    let value = world
        .record("probe")
        .and_then(|probe| probe.number("value"))
        .expect("the probe holds the value");
    [0, 1, 2].map(|index| value.component(index) * case.scale + case.bias)
}

#[test]
fn ops_compute_the_same_on_the_gpu_as_on_the_cpu() {
    let library = library();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let uv = builder.input(Input::Uv);
    let across = builder.swizzle(uv.clone(), "x");
    let across = builder.multiply(across, COLUMNS as f32);
    let across = builder.op(Op::Floor, [across.into()]);
    let down = builder.swizzle(uv, "y");
    let down = builder.multiply(down, ROWS as f32);
    let down = builder.op(Op::Floor, [down.into()]);
    let down = builder.multiply(down, COLUMNS as f32);
    let cell = builder.add(across, down);
    let mut total: Option<Wire> = None;
    for (index, case) in CASES.iter().enumerate() {
        let value = op(&mut builder, case);
        let value = builder.multiply(value, case.scale);
        let value = builder.add(value, Arg::from([case.bias; 3]));
        let chosen = builder.op(
            Op::Compare,
            [
                cell.clone().into(),
                (index as f32).into(),
                0.5.into(),
                4.0.into(),
            ],
        );
        let value = builder.multiply(value, chosen);
        total = Some(match total {
            Some(sum) => builder.add(sum, value),
            None => value,
        });
    }
    let glow = builder.shader(
        NodeKind::Emission,
        [("color", total.expect("cases exist").into())],
    );
    builder.output([("surface", glow.into())]);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());

    let mut renderer = renderer();
    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let screen = quad(&mut renderer, SCREEN_HALF);
    let mut frame = frame();
    frame.draw(Draw::new(screen, drawn));
    let shot = pixels(&mut renderer, &frame);

    let width = SIZE as f32 / COLUMNS as f32;
    let height = SIZE as f32 / ROWS as f32;
    for (index, case) in CASES.iter().enumerate() {
        let x = ((index % COLUMNS) as f32 + 0.5) * width;
        let y = ((index / COLUMNS) as f32 + 0.5) * height;
        let expected = on_cpu(&library, case).map(encoded);
        let found = rgb(&shot, x as u32, y as u32);
        assert!(
            near(found, expected, 2),
            "{:?} ({index}) draws {found:?} on the GPU but computes {expected:?} on the CPU",
            case.op
        );
    }
}
