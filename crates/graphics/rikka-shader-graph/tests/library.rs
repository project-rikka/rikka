use rikka_shader_graph::{
    Arg, Behavior, Builder, Bypass, GraphKind, Hint, Library, NodeKind, PortDefault, PortType,
    Problem,
};

const FAMILIES: &[&str] = &["npr.", "light.", "color.", "vector.", "bsdf.", "texture."];

fn arguments(code: &str) -> Vec<&str> {
    code.split('{')
        .skip(1)
        .filter_map(|part| part.split_once('}').map(|(name, _)| name))
        .collect()
}

#[test]
fn library_specs_are_consistent() {
    let library = Library::standard();
    for spec in library.specs() {
        assert!(
            FAMILIES.iter().any(|family| spec.id.starts_with(family)),
            "{} has no family prefix",
            spec.id
        );
        assert!(
            !spec.doc.is_empty() && !spec.title.is_empty(),
            "{} is undocumented",
            spec.id
        );
        for port in &spec.inputs {
            if let (Hint::Options(options), PortDefault::Value(chosen)) = (port.hint, port.default)
            {
                assert!(
                    (chosen.component(0) as usize) < options.len(),
                    "{}.{} defaults past its options",
                    spec.id,
                    port.name
                );
            }
        }
        match &spec.behavior {
            Behavior::Function(function) => {
                let used = arguments(function.code);
                for name in &used {
                    assert!(
                        spec.inputs.iter().any(|port| port.name == *name),
                        "{} reads unknown input {name}",
                        spec.id
                    );
                }
                for port in &spec.inputs {
                    assert!(
                        used.contains(&port.name),
                        "{} ignores input {}",
                        spec.id,
                        port.name
                    );
                }
                if !function.outputs.is_empty() {
                    let members: Vec<&str> =
                        function.outputs.iter().map(|(port, _)| *port).collect();
                    let outputs: Vec<&str> = spec.outputs.iter().map(|port| port.name).collect();
                    assert_eq!(members, outputs, "{} unpacks other outputs", spec.id);
                }
                if let Some(gate) = function.gate {
                    let [output] = spec.outputs.as_slice() else {
                        panic!("{} gates more than one output", spec.id);
                    };
                    let input = |name: &str| spec.inputs.iter().find(|port| port.name == name);
                    assert!(
                        input(gate.input).is_some(),
                        "{} gates on unknown input {}",
                        spec.id,
                        gate.input
                    );
                    match gate.bypass {
                        Bypass::Input(name) => assert_eq!(
                            input(name).map(|port| port.ty),
                            Some(output.ty),
                            "{} bypasses to {name}, not an input of its output type",
                            spec.id
                        ),
                        Bypass::Constant(constant) => assert_eq!(
                            PortType::Value(constant.ty()),
                            output.ty,
                            "{} bypasses to a constant of another type",
                            spec.id
                        ),
                    }
                }
            }
            Behavior::Closure(closure) => {
                let fields: Vec<&str> = closure.fields.iter().map(|(port, _)| *port).collect();
                let inputs: Vec<&str> = spec.inputs.iter().map(|port| port.name).collect();
                assert_eq!(fields, inputs, "{} leaves inputs unrouted", spec.id);
            }
            Behavior::Filter | Behavior::Record => {}
        }
    }
}

fn problems(build: impl FnOnce(&mut Builder<'_>)) -> Vec<Problem> {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    build(&mut builder);
    match builder.finish() {
        Ok(_) => Vec::new(),
        Err(error) => error
            .errors()
            .iter()
            .map(|error| error.problem.clone())
            .collect(),
    }
}

#[test]
fn nodes_defaulting_to_the_light_run_per_light() {
    let emitted = problems(|builder| {
        let facing = builder.external("npr.wrap", []);
        let facing = builder.out(facing, "value");
        let glow = builder.shader(NodeKind::Emission, [("strength", facing.into())]);
        builder.output([("surface", glow.into())]);
    });
    assert_eq!(emitted, [Problem::PerLight]);

    let lit = problems(|builder| {
        let facing = builder.external("npr.wrap", []);
        let facing = builder.out(facing, "value");
        let shader = builder.shader(NodeKind::Lighting, [("light", facing.into())]);
        builder.output([("surface", shader.into())]);
    });
    assert!(lit.is_empty(), "{lit:?}");

    let fixed = problems(|builder| {
        let facing = builder.external("npr.wrap", [("light", Arg::from([0.0, 1.0, 0.0]))]);
        let facing = builder.out(facing, "value");
        let glow = builder.shader(NodeKind::Emission, [("strength", facing.into())]);
        builder.output([("surface", glow.into())]);
    });
    assert!(
        fixed.is_empty(),
        "a set light direction is not per light: {fixed:?}"
    );
}

#[test]
fn outline_direction_belongs_to_the_outline() {
    let found = problems(|builder| {
        let direction = builder.external("npr.outline_direction", []);
        let direction = builder.out(direction, "value");
        let glow = builder.shader(NodeKind::Emission, [("color", direction.into())]);
        builder.output([("surface", glow.into())]);
    });
    assert!(
        found
            .iter()
            .any(|problem| matches!(problem, Problem::DefaultUnavailable { .. })),
        "{found:?}"
    );
}
