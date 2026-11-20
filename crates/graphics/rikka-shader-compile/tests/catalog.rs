use rikka_shader_graph::{
    Arg, Behavior, Builder, Graph, GraphKind, Input, Library, NodeKind, Op, PIXEL_ENTRY,
    ParameterDecl, PortDefault, PortType, Ports, PositionSpace, Scope, SocketValue, Step,
    ValueType, Wire,
};
use rikka_slang::Role;

use crate::support::{compiled, compiler};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Site {
    Surface,
    Light,
    Ambient,
    Combine,
    Displacement,
    Outline,
    Composite,
}

impl Site {
    fn graph(self) -> GraphKind {
        match self {
            Self::Composite => GraphKind::Composite,
            _ => GraphKind::Material,
        }
    }
}

#[derive(Clone, Copy)]
enum Settings {
    Folded,
    Live,
}

const WORLD_POSITION: PortDefault = PortDefault::Input(Input::Position(PositionSpace::World));

fn numeric_output(ports: &Ports) -> Option<(String, Option<ValueType>)> {
    ports.outputs.iter().find_map(|port| match port.ty {
        PortType::Dynamic => Some((port.name.clone(), None)),
        PortType::Value(ty) if ty.is_number() => Some((port.name.clone(), Some(ty))),
        PortType::Value(_) => None,
    })
}

fn feeding(builder: &mut Builder<'_>, site: Site) -> Option<Wire> {
    (site == Site::Combine).then(|| {
        let base = builder.external("bsdf.diffuse", []);
        let base = builder.out(base, "shader");
        let seen = builder.with(NodeKind::ShaderToRgb, [("shader", base.into())]);
        builder.out(seen, "color")
    })
}

fn place(
    builder: &mut Builder<'_>,
    library: &Library,
    site: Site,
    node: &NodeKind,
    settings: Settings,
    feed: Option<&Wire>,
) -> Option<Wire> {
    let ports = Graph::new(site.graph())
        .ports(library, &Scope::Main, node)
        .ok()?;
    let (output, ty) = numeric_output(&ports)?;
    let fed = match feed {
        Some(_) => Some(ports.inputs.iter().find(|port| match port.ty {
            PortType::Dynamic => true,
            PortType::Value(ty) => ty.width().is_some_and(|width| width <= 3),
        })?),
        None => None,
    };
    let id = builder.node(node.clone());
    for port in &ports.inputs {
        if let (Settings::Live, PortDefault::Value(constant)) = (settings, port.default) {
            builder.set(id, &port.name, Arg::Value(SocketValue::Number(constant)));
        }
        if site == Site::Composite && port.default == WORLD_POSITION {
            let uv = builder.input(Input::Uv);
            builder.set(id, &port.name, uv);
        }
    }
    if let (Some(fed), Some(feed)) = (fed, feed) {
        builder.set(id, &fed.name, feed);
    }
    let wire = builder.out(id, &output);
    Some(match ty {
        Some(ValueType::Float2) => builder.swizzle(wire, "x"),
        Some(ValueType::Float4) => builder.swizzle(wire, "xyz"),
        _ => wire,
    })
}

fn sink(builder: &mut Builder<'_>, site: Site, value: Wire) {
    let value = builder.add(value, Arg::from([0.0, 0.0, 0.0]));
    match site {
        Site::Surface | Site::Combine => {
            let glow = builder.shader(NodeKind::Emission, [("color", value.into())]);
            builder.output([("surface", glow.into())]);
        }
        Site::Light | Site::Ambient => {
            let port = if site == Site::Light {
                "light"
            } else {
                "ambient"
            };
            let lit = builder.shader(NodeKind::Lighting, [(port, value.into())]);
            builder.output([("surface", lit.into())]);
        }
        Site::Displacement => {
            builder.parameter(ParameterDecl::float("reach", 0.1).with_role(Role::Displacement));
            builder.output([("displacement", value.into())]);
        }
        Site::Outline => {
            let body = builder.shader(NodeKind::Emission, []);
            let line = builder.shader(NodeKind::Emission, [("color", [1.0, 0.0, 0.0].into())]);
            let width = builder.swizzle(value, "x");
            builder.output([
                ("surface", body.into()),
                ("outline", line.into()),
                ("outline_width", width.into()),
            ]);
        }
        Site::Composite => {
            builder.output([("image", value.into())]);
        }
    }
}

fn graph(library: &Library, site: Site, nodes: &[NodeKind]) -> Option<Graph> {
    let mut builder = Builder::new(site.graph(), library);
    let feed = feeding(&mut builder, site);
    let mut total: Option<Wire> = None;
    for (index, node) in nodes.iter().enumerate() {
        let settings = match index % 2 {
            0 => Settings::Folded,
            _ => Settings::Live,
        };
        let value = place(&mut builder, library, site, node, settings, feed.as_ref())?;
        total = Some(match total {
            Some(sum) => builder.add(sum, value),
            None => value,
        });
    }
    sink(&mut builder, site, total?);
    builder.finish().ok()
}

fn hosted(library: &Library, site: Site, node: &NodeKind) -> bool {
    graph(library, site, std::slice::from_ref(node)).is_some()
}

fn functions(library: &Library) -> Vec<NodeKind> {
    library
        .specs()
        .filter(|spec| matches!(spec.behavior, Behavior::Function(_)))
        .map(|spec| NodeKind::Extern(spec.id.to_owned()))
        .collect()
}

fn ops() -> Vec<NodeKind> {
    Op::ALL.into_iter().map(NodeKind::Op).collect()
}

fn named(nodes: &[NodeKind]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| match node {
            NodeKind::Extern(id) => id.clone(),
            NodeKind::Op(op) => format!("{op:?}"),
            other => format!("{other:?}"),
        })
        .collect()
}

fn compiles(
    site: Site,
    candidates: Vec<NodeKind>,
    keep: impl Fn(&NodeKind) -> bool,
    expected: &[&str],
) {
    let library = Library::standard();
    let nodes: Vec<NodeKind> = candidates
        .into_iter()
        .filter(|node| hosted(&library, site, node) && keep(node))
        .collect();
    let names = named(&nodes);
    for name in expected {
        assert!(
            names.iter().any(|known| known == name),
            "{name} is missing from {site:?}: {names:?}"
        );
    }
    let graph = graph(&library, site, &nodes).expect("the nodes combine into one valid graph");
    match site {
        Site::Composite => {
            let plan = graph.composite(&library).expect("the composite plans");
            let compiler = compiler();
            for (index, step) in plan.steps.iter().enumerate() {
                let Step::Pixel(pass) = step else {
                    continue;
                };
                if let Err(error) =
                    compiler.effect_code(&format!("pass{index}"), &pass.source, PIXEL_ENTRY)
                {
                    panic!("{error}\n{}", pass.source);
                }
            }
        }
        _ => {
            compiled(&graph, &library, &compiler());
        }
    }
}

fn surface_only(library: &Library) -> impl Fn(&NodeKind) -> bool + '_ {
    move |node| !hosted(library, Site::Surface, node)
}

#[test]
fn library_nodes_compile_in_the_surface() {
    compiles(
        Site::Surface,
        functions(&Library::standard()),
        |_| true,
        &[
            "npr.toon_step",
            "npr.ramp",
            "npr.face_sample",
            "npr.depth_rim",
            "color.tone_curve",
            "vector.bump",
        ],
    );
}

#[test]
fn ops_compile_in_the_surface() {
    compiles(
        Site::Surface,
        ops(),
        |_| true,
        &["MapRange", "Brick", "Wavelength"],
    );
}

#[test]
fn per_light_nodes_compile_in_the_light_loop() {
    let library = Library::standard();
    compiles(
        Site::Light,
        functions(&library),
        surface_only(&library),
        &[
            "light.sample",
            "light.shadow",
            "npr.face_sdf",
            "npr.kajiya_kay",
            "npr.hair_shadow",
            "npr.cast_gate",
        ],
    );
}

#[test]
fn ambient_nodes_compile_in_the_ambient_round() {
    let library = Library::standard();
    let mut candidates = functions(&library);
    candidates.extend(ops());
    compiles(
        Site::Ambient,
        candidates,
        surface_only(&library),
        &["light.indirect", "light.environment", "SkyIrradiance"],
    );
}

#[test]
fn library_nodes_compile_after_a_closure() {
    compiles(
        Site::Combine,
        functions(&Library::standard()),
        |_| true,
        &["npr.toon_step", "npr.shade_mix", "color.luminance_add"],
    );
}

#[test]
fn library_nodes_compile_in_the_displacement() {
    let mut candidates = functions(&Library::standard());
    candidates.extend(ops());
    compiles(
        Site::Displacement,
        candidates,
        |_| true,
        &["npr.flatten", "npr.outline_width", "MapRange"],
    );
}

#[test]
fn outline_nodes_compile_in_the_hull() {
    let library = Library::standard();
    compiles(
        Site::Outline,
        functions(&library),
        |node| {
            matches!(node, NodeKind::Extern(id) if id.starts_with("npr.outline"))
                || !hosted(&library, Site::Displacement, node)
        },
        &[
            "npr.outline_width",
            "npr.outline_direction",
            "npr.outline_depth",
            "npr.outline_fade",
        ],
    );
}

#[test]
fn pixel_nodes_compile_in_composites() {
    let mut candidates = functions(&Library::standard());
    candidates.extend(ops());
    compiles(
        Site::Composite,
        candidates,
        |_| true,
        &[
            "color.luminance_add",
            "color.tone_curve",
            "MapRange",
            "Checker",
            "Blackbody",
        ],
    );
}

#[test]
fn every_library_node_compiles_somewhere() {
    let library = Library::standard();
    let sites = [
        Site::Surface,
        Site::Light,
        Site::Ambient,
        Site::Combine,
        Site::Displacement,
        Site::Outline,
    ];
    let homeless: Vec<String> = named(&functions(&library))
        .into_iter()
        .zip(functions(&library))
        .filter(|(_, node)| !sites.iter().any(|site| hosted(&library, *site, node)))
        .map(|(name, _)| name)
        .collect();
    assert!(homeless.is_empty(), "no site hosts {homeless:?}");
}

fn closures_compile(ids: &[&str]) {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let mut total: Option<Wire> = None;
    for id in ids {
        let node = builder.external(id, []);
        let shader = builder.out(node, "shader");
        total = Some(match total {
            Some(sum) => builder.shader(
                NodeKind::AddShader,
                [("a", sum.into()), ("b", shader.into())],
            ),
            None => shader,
        });
    }
    builder.output([("surface", total.expect("closures exist").into())]);
    let graph = builder.finish().expect("closures add into one surface");
    compiled(&graph, &library, &compiler());
}

#[test]
fn reflection_closures_compile_together() {
    closures_compile(&["bsdf.diffuse", "bsdf.glossy", "bsdf.sheen"]);
}

#[test]
fn translucent_and_toon_closures_compile_together() {
    closures_compile(&["bsdf.translucent", "bsdf.toon"]);
}

#[test]
fn skin_closure_compiles() {
    closures_compile(&["bsdf.skin"]);
}

#[test]
fn hair_and_eye_closures_compile() {
    closures_compile(&["bsdf.hair", "bsdf.eye"]);
}
