use std::collections::BTreeMap;

use animate_look::{Look, Plans, Resolver, World, shared_library, world_outputs};
use rikka_gpu::TextureId;
use rikka_renderer_3d::{
    Bounds, LightProfileId, Literal, PanoramaSun, ParameterKind, ParameterSpec, TextureBinding,
    TextureKind,
};
use rikka_shader_graph::{
    Builder, Env, Graph, GraphKind, Library, NodeKind, SocketRef, SocketValue,
};

pub(crate) fn library() -> &'static Library {
    shared_library()
}

pub(crate) fn spec(name: &str, kind: ParameterKind, default: Literal) -> ParameterSpec {
    ParameterSpec {
        name: name.to_owned(),
        kind,
        offset: 0,
        default,
        range: Some(Bounds {
            low: 0.0,
            high: 1.0,
        }),
        soft_range: None,
        group: None,
        order: 0,
        label: None,
        hints: Vec::new(),
        presets: Vec::new(),
        feature: None,
        role: None,
    }
}

#[derive(Default)]
pub(crate) struct Lookup {
    pub(crate) parameters: BTreeMap<String, ParameterSpec>,
    pub(crate) textures: Vec<(String, TextureKind)>,
    pub(crate) panoramas: Vec<String>,
}

impl Resolver for Lookup {
    fn texture(&mut self, path: &str, kind: TextureKind) -> Option<TextureBinding> {
        self.textures.push((path.to_owned(), kind));
        None
    }

    fn panorama(&mut self, path: &str) -> Option<TextureId> {
        self.panoramas.push(path.to_owned());
        None
    }

    fn panorama_sun(&mut self, _: &str, _: f32) -> Option<PanoramaSun> {
        None
    }

    fn profile(&mut self, _: &str) -> Option<LightProfileId> {
        None
    }

    fn parameter(&mut self, name: &str) -> Option<ParameterSpec> {
        self.parameters.get(name).cloned()
    }
}

pub(crate) fn output_of(node: &str) -> &'static str {
    let produced = library().get(node).expect("a world node").outputs[0].ty;
    world_outputs()
        .into_iter()
        .find(|port| port.ty == produced)
        .expect("every world node feeds an output input")
        .name
}

pub(crate) fn wired(nodes: &[(&str, Vec<(&str, SocketValue)>)]) -> Graph {
    let library = library();
    let mut builder = Builder::new(GraphKind::World, library);
    let output = builder.node(NodeKind::Output);
    for (id, sockets) in nodes {
        let node = builder.external(id, []);
        for (port, value) in sockets {
            builder.set(node, port, rikka_shader_graph::Arg::Value(value.clone()));
        }
        let produced = &library.get(id).expect("a world node").outputs[0];
        let wire = builder.out(node, produced.name);
        builder.set(output, output_of(id), wire);
    }
    builder.finish().expect("a valid world")
}

pub(crate) fn world_of(graph: &Graph) -> World {
    world_at(graph, 0.0)
}

pub(crate) fn world_at(graph: &Graph, time: f32) -> World {
    let plan = graph.world(library()).expect("it plans");
    let sockets = |at: &SocketRef| graph.socket(at).cloned();
    World::from_record(&plan.evaluate(&Env {
        time,
        sockets: &sockets,
    }))
    .expect("a typed world")
}

pub(crate) fn evaluated_world(look: &Look, frame: f32) -> World {
    Plans::default()
        .evaluate(look, library(), frame, frame / 30.0)
        .expect("the look evaluates")
        .world
}
