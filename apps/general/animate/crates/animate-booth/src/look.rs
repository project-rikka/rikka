use animate_look::{GraphRef, Look, LookEdit, shared_library};
use rikka_shader_graph::{Builder, Graph, GraphKind, NodeId};

const BACKDROP: [f32; 3] = [0.022, 0.023, 0.026];
const FLOOR_SHADOW: f32 = 0.6;

fn wire(builder: &mut Builder<'_>, output: NodeId, port: &str, id: &str, inputs: Vec<(&str, f32)>) {
    let node = builder.external(id, []);
    for (input, value) in inputs {
        builder.set(node, input, value);
    }
    let out = builder.out(node, port);
    builder.set(output, port, out);
}

fn world() -> Result<Graph, rikka_shader_graph::Error> {
    let library = shared_library();
    let mut builder = Builder::new(GraphKind::World, library);
    let output = builder.output([]);
    let backdrop = builder.external("world.backdrop", [("color", BACKDROP.into())]);
    let environment = builder.out(backdrop, "environment");
    builder.set(output, "environment", environment);
    wire(
        &mut builder,
        output,
        "stage_light",
        "world.stage_light",
        Vec::new(),
    );
    wire(&mut builder, output, "shadows", "world.shadows", Vec::new());
    wire(
        &mut builder,
        output,
        "floor",
        "world.floor",
        vec![("shadow", FLOOR_SHADOW), ("grid", 0.0)],
    );
    builder.finish()
}

pub(crate) fn look() -> Look {
    let mut look = Look::default();
    let graph = world().expect("the booth world uses only built-in nodes");
    look.apply(
        shared_library(),
        LookEdit::Replace {
            graph: GraphRef::World,
            with: graph.into(),
        },
    )
    .expect("the booth world is a world graph");
    look
}
