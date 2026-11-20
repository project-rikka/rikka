use std::collections::BTreeSet;

use animate_shader::{
    PortSide, add_function_port, collapse, expand, remove_function, remove_function_port,
    rename_function,
};
use rikka_shader_graph::{
    Builder, Env, Graph, GraphKind, Input, Library, NodeKind, Op, Scope, SocketRef, ValueType,
};

use crate::support::library;

fn intensity(graph: &Graph, library: &Library, time: f32) -> Option<f32> {
    let plan = graph.world(library).expect("the world plans");
    let world = plan.evaluate(&Env {
        time,
        sockets: &|at: &SocketRef| graph.socket(at).cloned(),
    });
    world.records("lights").first()?.float("intensity")
}

fn world(library: &Library) -> (Graph, BTreeSet<rikka_shader_graph::NodeId>) {
    let mut builder = Builder::new(GraphKind::World, library);
    let time = builder.input(Input::Time);
    let wave = builder.op(Op::Sine, [time.into()]);
    let scaled = builder.multiply(&wave, 3.0);
    let lifted = builder.add(&scaled, 0.5);
    let light = builder.external("world.light", [("intensity", (&lifted).into())]);
    let out = builder.out(light, "light");
    builder.output([("lights", out.into())]);
    let inner = [wave.node(), scaled.node(), lifted.node()].into();
    (builder.finish().expect("a valid world"), inner)
}

#[test]
fn grouping_into_a_function_and_back_keeps_what_the_graph_computes() {
    let library = library();
    let (mut graph, inner) = world(&library);
    let original = graph.clone();
    let times = [0.0, 0.7, 2.0];
    let expected: Vec<Option<f32>> = times
        .iter()
        .map(|time| intensity(&graph, &library, *time))
        .collect();

    let grouped = collapse(&mut graph, &library, &Scope::Main, &inner, "wave").unwrap();
    let function = graph.function("wave").expect("the function exists");
    assert_eq!(function.inputs.len(), 1);
    assert_eq!(function.outputs.len(), 1);
    assert_eq!(function.outputs[0].ty, ValueType::Float);
    assert_eq!(graph.body().nodes().len(), 4);
    assert!(graph.validate(&library).is_valid());
    for (time, value) in times.iter().zip(&expected) {
        assert_eq!(intensity(&graph, &library, *time), *value);
    }

    let collapsed = graph.clone();
    let expanded = expand(&mut graph, &library, &Scope::Main, grouped.value).unwrap();
    assert_eq!(expanded.value.len(), 3);
    assert!(graph.validate(&library).is_valid());
    for (time, value) in times.iter().zip(&expected) {
        assert_eq!(intensity(&graph, &library, *time), *value);
    }

    graph.apply(&library, expanded.undo).unwrap();
    assert_eq!(graph, collapsed);
    graph.apply(&library, grouped.undo).unwrap();
    assert_eq!(graph, original);
}

#[test]
fn renaming_and_removing_functions_follow_their_calls() {
    let library = library();
    let (mut graph, inner) = world(&library);
    let call = collapse(&mut graph, &library, &Scope::Main, &inner, "wave")
        .unwrap()
        .value;

    rename_function(&mut graph, &library, "wave", "pulse").unwrap();
    assert!(graph.function("wave").is_none());
    assert_eq!(
        graph.body().node(call).map(|node| node.kind.clone()),
        Some(NodeKind::Call("pulse".to_owned()))
    );
    assert!(graph.validate(&library).is_valid());

    let added = add_function_port(
        &mut graph,
        &library,
        "pulse",
        PortSide::Input,
        "Speed!",
        ValueType::Float,
    )
    .unwrap();
    assert_eq!(added.value, "speed_");
    let input = graph.function("pulse").unwrap().inputs[0].name.clone();
    let before = graph.clone();
    let dropped =
        remove_function_port(&mut graph, &library, "pulse", PortSide::Input, &input).unwrap();
    assert_eq!(graph.function("pulse").unwrap().inputs.len(), 1);
    graph.apply(&library, dropped.undo).unwrap();
    assert_eq!(graph, before);

    remove_function(&mut graph, &library, "pulse").unwrap();
    assert!(graph.functions().is_empty());
    assert!(graph.body().node(call).is_none());
}
