use rikka_shader_graph::{Builder, Env, GraphKind, Input, NodeKind, Op, SocketRef};

use crate::support::library;

#[test]
fn records_gather_into_the_output_with_driven_values() {
    let library = library();
    let mut builder = Builder::new(GraphKind::World, &library);
    let time = builder.input(Input::Time);
    let flicker = builder.op(Op::Sine, [time.into()]);
    let key = builder.external(
        "world.light",
        [
            ("intensity", flicker.into()),
            ("profile", "spot.ies".into()),
        ],
    );
    let fill = builder.external("world.light", [("color", [0.2, 0.4, 1.0].into())]);
    let sky = builder.external("world.sky", [("hour", 18.5.into())]);
    let (key_out, fill_out, sky_out) = (
        builder.out(key, "light"),
        builder.out(fill, "light"),
        builder.out(sky, "sky"),
    );
    let output = builder.node(NodeKind::Output);
    builder
        .set(output, "lights", key_out)
        .set(output, "lights", fill_out)
        .set(output, "sky", sky_out);
    let graph = builder.finish().expect("a valid world");
    let plan = graph.world(&library).expect("it plans");

    let world = plan.evaluate(&Env {
        time: std::f32::consts::FRAC_PI_2,
        sockets: &|at: &SocketRef| graph.socket(at).cloned(),
    });
    let lights = world.records("lights");
    assert_eq!(lights.len(), 2);
    assert_eq!(lights[0].id, "world.light");
    assert_eq!(lights[0].float("intensity"), Some(1.0));
    assert_eq!(lights[0].asset("profile"), Some("spot.ies"));
    assert_eq!(
        lights[1].number("color").map(|c| c.values().to_vec()),
        Some(vec![0.2, 0.4, 1.0])
    );
    assert_eq!(lights[1].asset("profile"), None);
    assert_eq!(
        world.record("sky").and_then(|sky| sky.float("hour")),
        Some(18.5)
    );
}

#[test]
fn unlinked_records_are_absent() {
    let library = library();
    let mut builder = Builder::new(GraphKind::World, &library);
    builder.node(NodeKind::Output);
    let graph = builder.finish().expect("an empty world is valid");
    let world = graph.world(&library).expect("it plans").evaluate(&Env {
        time: 0.0,
        sockets: &|_| None,
    });
    assert!(world.record("sky").is_none());
    assert!(world.records("lights").is_empty());
}
