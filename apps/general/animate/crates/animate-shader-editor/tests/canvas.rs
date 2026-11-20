use animate_shader::{Diagnostic, Location};
use animate_shader_editor::{node_graph, node_key};
use rikka_shader_graph::{Builder, GraphKind, Layer, NodeKind, Scope};
use rikka_widgets::node_canvas::{CanvasLink, PortAddr};

use crate::support::library;

#[test]
fn list_inputs_carry_every_link_and_set_values_show_on_their_port() {
    let library = library();
    let mut builder = Builder::new(GraphKind::World, &library);
    let key = builder.external("world.light", [("intensity", 4.0.into())]);
    let fill = builder.external("world.light", []);
    let (key_out, fill_out) = (builder.out(key, "light"), builder.out(fill, "light"));
    let output = builder.node(NodeKind::Output);
    builder
        .set(output, "lights", key_out)
        .set(output, "lights", fill_out);
    let graph = builder.finish().unwrap();

    let canvas = node_graph(&graph, &library, &Scope::Main, &[]).unwrap();
    let world = canvas
        .nodes
        .iter()
        .find(|node| node.key == node_key(output))
        .unwrap();
    assert!(world.inputs[0].many);
    let into: Vec<&CanvasLink> = canvas
        .links
        .iter()
        .filter(|link| link.to == PortAddr::input(node_key(output), 0))
        .collect();
    assert_eq!(into.len(), 2);
    let light = canvas
        .nodes
        .iter()
        .find(|node| node.key == node_key(key))
        .unwrap();
    assert_eq!(light.title, "Light");
    assert_eq!(light.inputs[1].label, "intensity = 4");
    assert!(canvas.nodes.iter().all(|node| node.problem.is_none()));
}

#[test]
fn problems_from_validation_and_diagnostics_badge_their_nodes() {
    let library = library();
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let image = builder.layer(Layer::Image);
    let output = builder.output([("image", image.clone().into())]);
    let mut graph = builder.finish().unwrap();
    let sample = graph
        .edit(&library, Scope::Main)
        .add_node(NodeKind::Sample, [0.0, 200.0])
        .unwrap()
        .value;
    let diagnostics = [Diagnostic {
        at: Location::Node {
            scope: Scope::Main,
            node: output,
            port: None,
        },
        message: "too dark".to_owned(),
    }];

    let canvas = node_graph(&graph, &library, &Scope::Main, &diagnostics).unwrap();
    let problem = |id| {
        canvas
            .nodes
            .iter()
            .find(|node| node.key == node_key(id))
            .and_then(|node| node.problem.clone())
    };
    assert!(problem(sample).is_some_and(|problem| problem.starts_with("image: ")));
    assert_eq!(problem(output).as_deref(), Some("too dark"));
    assert_eq!(problem(image.node()), None);
}
