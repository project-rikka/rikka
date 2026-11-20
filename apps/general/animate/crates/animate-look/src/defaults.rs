use rikka_shader_graph::{Builder, Graph, GraphKind, Layer, Library, NodeId};

use crate::error::Error;
use crate::graph_ref::GraphRef;
use crate::world::nodes::{
    BACKDROP, ENVIRONMENT, FLOOR, FLOOR_NODE, SAMPLING, SAMPLING_NODE, SHADOWS, SHADOWS_NODE, SKY,
    STAGE_LIGHT, STAGE_LIGHT_NODE,
};

const MANUAL_METERING: f32 = 1.0;
const CLAMP_TONE: f32 = 4.0;
const STANDARD_FILTERS: [&str; 7] = [
    "filter.exposure",
    "filter.motion_blur",
    "filter.depth_of_field",
    "filter.glow",
    "filter.grade",
    "filter.tonemap",
    "filter.lens",
];

fn finished(builder: Builder<'_>, graph: GraphRef) -> Result<Graph, Error> {
    builder
        .finish()
        .map_err(|source| Error::Graph { graph, source })
}

fn wire(builder: &mut Builder<'_>, output: NodeId, port: &str, node: NodeId, from: &str) {
    let wire = builder.out(node, from);
    builder.set(output, port, wire);
}

fn stage(builder: &mut Builder<'_>, output: NodeId) {
    let floor = builder.external(FLOOR_NODE, []);
    wire(builder, output, FLOOR, floor, FLOOR);
    let sampling = builder.external(SAMPLING_NODE, []);
    wire(builder, output, SAMPLING, sampling, SAMPLING);
}

pub fn default_world(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::World, library);
    let output = builder.output([]);
    let backdrop = builder.external(BACKDROP, []);
    wire(&mut builder, output, ENVIRONMENT, backdrop, ENVIRONMENT);
    let light = builder.external(STAGE_LIGHT_NODE, []);
    wire(&mut builder, output, STAGE_LIGHT, light, STAGE_LIGHT);
    let shadows = builder.external(SHADOWS_NODE, []);
    wire(&mut builder, output, SHADOWS, shadows, SHADOWS);
    stage(&mut builder, output);
    finished(builder, GraphRef::World)
}

pub fn cinematic_world(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::World, library);
    let output = builder.output([]);
    let sky = builder.external(SKY, []);
    wire(&mut builder, output, ENVIRONMENT, sky, ENVIRONMENT);
    let shadows = builder.external(SHADOWS_NODE, [("contact", 1.0.into())]);
    wire(&mut builder, output, SHADOWS, shadows, SHADOWS);
    for (port, id) in [
        ("fog", "world.fog"),
        ("occlusion", "world.occlusion"),
        ("reflections", "world.reflections"),
        ("illumination", "world.illumination"),
    ] {
        let node = builder.external(id, []);
        wire(&mut builder, output, port, node, port);
    }
    stage(&mut builder, output);
    finished(builder, GraphRef::World)
}

pub fn default_composite(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Composite, library);
    let image = builder.layer(Layer::Image);
    let exposure = builder.external(
        "filter.exposure",
        [
            ("image", image.into()),
            ("metering", MANUAL_METERING.into()),
        ],
    );
    let exposed = builder.out(exposure, "image");
    let tonemap = builder.external(
        "filter.tonemap",
        [("image", exposed.into()), ("tone", CLAMP_TONE.into())],
    );
    let mapped = builder.out(tonemap, "image");
    builder.output([("image", mapped.into())]);
    finished(builder, GraphRef::Composite)
}

pub fn cinematic_composite(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Composite, library);
    let mut image = builder.layer(Layer::Image);
    for id in STANDARD_FILTERS {
        let filter = builder.external(id, [("image", image.into())]);
        image = builder.out(filter, "image");
    }
    builder.output([("image", image.into())]);
    finished(builder, GraphRef::Composite)
}
