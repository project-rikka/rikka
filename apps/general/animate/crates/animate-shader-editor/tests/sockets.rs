use animate_shader::empty_material;
use animate_shader_editor::{Fallback, Field, SocketState, socket_rows};
use rikka_shader_graph::{Constant, DirectionSpace, Input, NodeKind, PortRef, Scope, SocketValue};

use crate::support::library;

#[test]
fn rows_tell_linked_set_and_default_sockets_apart() {
    let library = library();
    let mut graph = empty_material(&library).unwrap();
    let find = |graph: &rikka_shader_graph::Graph, wanted: fn(&NodeKind) -> bool| {
        *graph
            .body()
            .nodes()
            .iter()
            .find(|(_, node)| wanted(&node.kind))
            .unwrap()
            .0
    };
    let bsdf = find(&graph, |kind| matches!(kind, NodeKind::Extern(_)));
    let output = find(&graph, |kind| *kind == NodeKind::Output);
    let mut editor = graph.edit(&library, Scope::Main);
    editor
        .set_input(bsdf, "base_color", Some([0.9, 0.1, 0.1].into()))
        .unwrap();
    let constant = editor
        .add_node(NodeKind::Constant(Constant::scalar(0.2)), [0.0, 0.0])
        .unwrap()
        .value;
    editor
        .connect(
            PortRef::new(constant, "value"),
            PortRef::new(bsdf, "roughness"),
        )
        .unwrap();

    let rows = socket_rows(&graph, &library, &Scope::Main, bsdf);
    let row = |port: &str| rows.iter().find(|row| row.port == port).unwrap();
    assert_eq!(row("base_color").field, Field::Color { alpha: false });
    assert_eq!(
        row("base_color").state,
        SocketState::Set(SocketValue::from([0.9, 0.1, 0.1]))
    );
    let SocketState::Linked(sources) = &row("roughness").state else {
        panic!("roughness is linked: {:?}", row("roughness").state);
    };
    assert_eq!(
        (sources[0].node, sources[0].title.as_str()),
        (constant, "Number")
    );
    assert_eq!(
        row("metalness").field,
        Field::Number {
            range: Some((0.0, 1.0)),
            hard: true,
            integer: false,
            angle: false,
        }
    );
    assert_eq!(
        row("metalness").fallback_value(),
        Some(SocketValue::from(0.0))
    );
    assert_eq!(
        row("normal").state,
        SocketState::Default(Fallback::Context(Input::Normal(DirectionSpace::World)))
    );

    let outputs = socket_rows(&graph, &library, &Scope::Main, output);
    let surface = outputs.iter().find(|row| row.port == "surface").unwrap();
    assert_eq!(surface.field, Field::Link);
    assert!(matches!(surface.state, SocketState::Linked(_)));
    let outline = outputs.iter().find(|row| row.port == "outline").unwrap();
    assert_eq!(outline.state, SocketState::Default(Fallback::Empty));
}
