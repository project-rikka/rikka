use rikka_shader_graph::{
    Builder, Edit, Graph, GraphKind, Input, Layer, NodeKind, Op, PortRef, Problem, Scope,
};

use crate::support::library;

#[test]
fn every_edit_returns_the_edit_that_undoes_it() {
    let library = library();
    let mut graph = Graph::new(GraphKind::Composite);
    let mut history: Vec<(Graph, Edit)> = Vec::new();

    let before = graph.clone();
    let scene = graph
        .edit(&library, Scope::Main)
        .add_node(NodeKind::Layer(Layer::Image), [0.0, 0.0])
        .expect("a layer");
    history.push((before, scene.undo));
    let scene = scene.value;

    let before = graph.clone();
    let scale = graph
        .edit(&library, Scope::Main)
        .add_node(NodeKind::Op(Op::Multiply), [200.0, 0.0])
        .expect("an op");
    history.push((before, scale.undo));
    let scale = scale.value;

    for step in 0..4 {
        let before = graph.clone();
        let mut editor = graph.edit(&library, Scope::Main);
        let change = match step {
            0 => editor.connect(PortRef::new(scene, "image"), PortRef::new(scale, "a")),
            1 => editor.set_input(scale, "b", Some(0.5.into())),
            2 => editor.move_node(scale, [320.0, 40.0]),
            _ => editor.remove_node(scene),
        }
        .expect("the edit applies");
        history.push((before, change.undo));
    }

    while let Some((before, undo)) = history.pop() {
        let after = graph.clone();
        let redo = graph.apply(&library, undo).expect("the undo applies");
        assert_eq!(graph, before);
        let mut again = graph.clone();
        again.apply(&library, redo).expect("the redo applies");
        assert_eq!(again, after);
    }
}

#[test]
fn list_inputs_take_many_links_and_single_inputs_replace() {
    let library = library();
    let mut graph = Graph::new(GraphKind::World);
    let mut editor = graph.edit(&library, Scope::Main);
    let output = editor
        .add_node(NodeKind::Output, [0.0, 0.0])
        .expect("an output")
        .value;
    let lights: Vec<_> = (0..2)
        .map(|_| {
            editor
                .add_node(NodeKind::Extern("world.light".to_owned()), [0.0, 0.0])
                .expect("a light")
                .value
        })
        .collect();
    for light in &lights {
        editor
            .connect(
                PortRef::new(*light, "light"),
                PortRef::new(output, "lights"),
            )
            .expect("a list takes every light");
    }
    let again = editor.connect(
        PortRef::new(lights[0], "light"),
        PortRef::new(output, "lights"),
    );
    assert_eq!(
        again.expect_err("the same link twice").problem,
        Problem::AlreadyConnected
    );
    editor
        .detach(
            PortRef::new(lights[0], "light"),
            PortRef::new(output, "lights"),
        )
        .expect("one link leaves");
    let to = PortRef::new(output, "lights");
    let left: Vec<_> = graph.body().sources(&to).map(|from| from.node).collect();
    assert_eq!(left, [lights[1]]);
}

#[test]
fn graphs_survive_a_json_round_trip() {
    let library = library();
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let scene = builder.layer(Layer::Image);
    let time = builder.input(Input::Time);
    let pulse = builder.op(Op::Sine, [time.into()]);
    let lit = builder.multiply(scene, pulse);
    builder.output([("image", lit.into())]);
    let graph = builder.finish().expect("a valid composite");

    let text = serde_json::to_string(&graph).expect("it serializes");
    let back: Graph = serde_json::from_str(&text).expect("it parses");
    assert_eq!(back, graph);
    assert!(back.validate(&library).is_valid());

    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Tagged {
        Held { graph: Graph },
    }
    let text = serde_json::to_string(&Tagged::Held {
        graph: graph.clone(),
    })
    .expect("it serializes");
    let Tagged::Held { graph: held } = serde_json::from_str(&text).expect("a tagged graph parses");
    assert_eq!(held, graph);
}

#[test]
fn built_graphs_are_laid_out_along_their_links() {
    let library = library();
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let scene = builder.layer(Layer::Image);
    let half = builder.multiply(scene, 0.5);
    builder.output([("image", half.into())]);
    let graph = builder.finish().expect("a valid composite");

    for connection in graph.body().connections() {
        let x = |id| graph.body().node(id).expect("a linked node").position[0];
        assert!(x(connection.from.node) < x(connection.to.node));
    }
}
