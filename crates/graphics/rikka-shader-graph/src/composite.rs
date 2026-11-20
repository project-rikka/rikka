use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::context::Site;
use crate::emit::{Emitter, Socket, Uniforms, Value, literal};
use crate::error::{Error, GraphError};
use crate::eval::{Compiler, Env, Program};
use crate::flat::{Flat, FlatNode, Path, Slot, Source};
use crate::graph::{Graph, GraphKind, NodeId};
use crate::library::{Behavior, Library};
use crate::material::texture_kind;
use crate::node::{Layer, NodeKind};
use crate::types::{Constant, ValueType};
use crate::world::{self, FieldPlan, Record, RecordPlan};

pub const PIXEL_ENTRY: &str = "rikka_graph_effect";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Link {
    Layer(Layer),
    Step(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilterStep {
    pub id: String,
    pub key: u64,
    pub path: Vec<NodeId>,
    pub images: Vec<(String, Link)>,
    values: Vec<(String, FieldPlan)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PixelStep {
    pub key: u64,
    pub source: String,
    pub images: Vec<(String, Link)>,
    pub sockets: Vec<Socket>,
    pub timed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Filter(FilterStep),
    Pixel(PixelStep),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompositePlan {
    program: Program,
    pub steps: Vec<Step>,
    pub output: Link,
}

impl CompositePlan {
    pub fn evaluate(&self, env: &Env<'_>) -> Vec<Option<Record>> {
        let mut values = Vec::new();
        self.program.run(env, &mut values);
        self.steps
            .iter()
            .map(|step| match step {
                Step::Filter(filter) => Some(world::resolve(
                    &RecordPlan {
                        id: filter.id.clone(),
                        path: filter.path.clone(),
                        fields: filter.values.clone(),
                    },
                    env,
                    &values,
                )),
                Step::Pixel(_) => None,
            })
            .collect()
    }
}

impl Graph {
    pub fn composite(&self, library: &Library) -> Result<CompositePlan, Error> {
        let report = self.validate(library);
        if !report.is_valid() || self.kind() != GraphKind::Composite {
            return Err(Error(report.errors));
        }
        let flat = Flat::new(self, library, &report);
        let (path, output) = flat.output().expect("a checked graph has an output");
        let mut builder = Builder {
            flat: &flat,
            compiler: Compiler::new(&flat),
            steps: Vec::new(),
            filters: BTreeMap::new(),
            pixels: BTreeMap::new(),
        };
        let output = builder.image(path, output, "image");
        Ok(CompositePlan {
            program: builder.compiler.finish(),
            steps: builder.steps,
            output,
        })
    }
}

enum Producer<'f, 'g> {
    Layer(Layer),
    Filter(&'f Path, &'f FlatNode<'g>),
}

fn producer<'f, 'g>(flat: &'f Flat<'g>, path: &Path) -> Option<Producer<'f, 'g>> {
    let (path, node) = flat.nodes.get_key_value(path)?;
    match node.kind {
        NodeKind::Layer(layer) => Some(Producer::Layer(*layer)),
        NodeKind::Extern(_) => matches!(flat.spec(node)?.behavior, Behavior::Filter)
            .then_some(Producer::Filter(path, node)),
        NodeKind::FunctionInput(_) | NodeKind::FunctionOutput(_)
            if flat.types(node).inputs.get("value") == Some(&ValueType::Image)
                || flat.types(node).outputs.get("value") == Some(&ValueType::Image) =>
        {
            match node.input("value") {
                Source::Link(from, _) => producer(flat, from),
                _ => None,
            }
        }
        _ => None,
    }
}

fn takes_image(flat: &Flat<'_>, node: &FlatNode<'_>, port: &str) -> bool {
    flat.types(node).inputs.get(port) == Some(&ValueType::Image)
}

fn hash(path: &[NodeId], tag: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut feed = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    for id in path {
        id.get().to_le_bytes().into_iter().for_each(&mut feed);
        feed(0xff);
    }
    tag.bytes().for_each(&mut feed);
    hash
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Root {
    Slot(Slot),
    Input(Path, String),
}

struct Builder<'f, 'g> {
    flat: &'f Flat<'g>,
    compiler: Compiler<'f, 'g>,
    steps: Vec<Step>,
    filters: BTreeMap<Path, usize>,
    pixels: BTreeMap<Root, usize>,
}

#[derive(Default)]
struct Cone {
    nodes: BTreeSet<Path>,
    loads: Vec<(Slot, Link)>,
    samples: Vec<(Path, Link)>,
}

impl<'f, 'g> Builder<'f, 'g> {
    fn image(&mut self, path: &Path, node: &FlatNode<'g>, port: &str) -> Link {
        match node.input(port) {
            Source::Link(from, output) => match self.produced(from) {
                Some(link) => link,
                None => Link::Step(self.pixel(Root::Slot((from.clone(), output.clone())))),
            },
            _ => Link::Step(self.pixel(Root::Input(path.clone(), port.to_owned()))),
        }
    }

    fn produced(&mut self, path: &Path) -> Option<Link> {
        Some(match producer(self.flat, path)? {
            Producer::Layer(layer) => Link::Layer(layer),
            Producer::Filter(path, node) => Link::Step(self.filter(path, node)),
        })
    }

    fn filter(&mut self, path: &Path, node: &FlatNode<'g>) -> usize {
        if let Some(step) = self.filters.get(path) {
            return *step;
        }
        let flat = self.flat;
        let images = flat
            .types(node)
            .ports
            .inputs
            .iter()
            .filter(|port| takes_image(flat, node, &port.name))
            .filter(|port| !matches!(node.input(&port.name), Source::Empty))
            .map(|port| (port.name.clone(), self.image(path, node, &port.name)))
            .collect();
        let id = match node.kind {
            NodeKind::Extern(id) => id.clone(),
            _ => unreachable!("filters are library nodes"),
        };
        self.steps.push(Step::Filter(FilterStep {
            key: hash(path, &id),
            id,
            path: path.clone(),
            images,
            values: world::fields(&mut self.compiler, flat, node),
        }));
        let step = self.steps.len() - 1;
        self.filters.insert(path.clone(), step);
        step
    }

    fn gather(&mut self, node: &FlatNode<'g>, port: &str, cone: &mut Cone) {
        let Source::Link(from, output) = node.input(port) else {
            return;
        };
        if let Some(link) = self.produced(from) {
            cone.loads.push(((from.clone(), output.clone()), link));
            return;
        }
        if !cone.nodes.insert(from.clone()) {
            return;
        }
        let flat = self.flat;
        let inner = &flat.nodes[from];
        let ports: Vec<&str> = inner.inputs.iter().map(|(port, _)| port.as_str()).collect();
        for inner_port in ports {
            if *inner.kind == NodeKind::Sample && inner_port == "image" {
                let link = self.image(from, inner, "image");
                cone.samples.push((from.clone(), link));
            } else {
                self.gather(inner, inner_port, cone);
            }
        }
    }

    fn pixel(&mut self, root: Root) -> usize {
        if let Some(step) = self.pixels.get(&root) {
            return *step;
        }
        let flat = self.flat;
        let mut cone = Cone::default();
        match &root {
            Root::Slot((path, _)) => {
                cone.nodes.insert(path.clone());
                let node = &flat.nodes[path];
                let ports: Vec<&str> = node.inputs.iter().map(|(port, _)| port.as_str()).collect();
                for port in ports {
                    if *node.kind == NodeKind::Sample && port == "image" {
                        let link = self.image(path, node, "image");
                        cone.samples.push((path.clone(), link));
                    } else {
                        self.gather(node, port, &mut cone);
                    }
                }
            }
            Root::Input(path, port) => self.gather(&flat.nodes[path], port, &mut cone),
        }

        let mut images: Vec<(String, Link)> = Vec::new();
        let mut member = |link: Link| -> String {
            match images.iter().find(|(_, known)| *known == link) {
                Some((name, _)) => name.clone(),
                None => {
                    let name = format!("i{}", images.len());
                    images.push((name.clone(), link));
                    name
                }
            }
        };
        let mut uniforms = Uniforms::default();
        let mut emitter = Emitter::new(flat, Site::Pixel, &mut uniforms);
        for (slot, link) in &cone.loads {
            let name = member(*link);
            emitter.seed(
                slot.clone(),
                Value::Code(
                    format!("rikka_effect_load(push.{name}, pixel)"),
                    ValueType::Float4,
                ),
            );
        }
        for path in flat.order(&cone.nodes) {
            let node = &flat.nodes[&path];
            if *node.kind != NodeKind::Sample {
                emitter.node(&path, node);
                continue;
            }
            let link = cone
                .samples
                .iter()
                .find(|(sample, _)| *sample == path)
                .map(|(_, link)| *link)
                .expect("a gathered sample");
            let name = member(link);
            emitter.begin(&path, node);
            let uv = emitter.text(node, "uv");
            let declaration = format!(
                "float4 {} = rikka_effect_sample(push.{name}, {uv});",
                emitter.local()
            );
            emitter.pack(&path, declaration);
        }
        let value = match &root {
            Root::Slot(slot) => emitter.found(slot).cloned().expect("an emitted root"),
            Root::Input(path, port) => emitter.argument(&flat.nodes[path], port),
        };
        let color = match &value {
            Value::Constant(constant) => literal(&widened(constant)),
            Value::Code(code, ty) => match ty.width() {
                Some(1) => format!("float4(float3({code}), 1.0)"),
                Some(2) => format!("float4({code}, 0.0, 1.0)"),
                Some(3) => format!("float4({code}, 1.0)"),
                _ => code.clone(),
            },
        };
        let timed = emitter.timed;
        let lines = std::mem::take(&mut emitter.lines);
        drop(emitter);

        let mut source = String::from("import rikka;\nimport rikka.graph;\n\n");
        let parameters = !uniforms.list.is_empty();
        let mut line = |depth: usize, text: &str| {
            writeln!(source, "{}{text}", "    ".repeat(depth)).expect("writing to a string");
        };
        if parameters {
            line(0, "struct Parameters {");
            for uniform in &uniforms.list {
                match uniform.ty {
                    ValueType::Texture => line(
                        1,
                        &format!(
                            "[Texture(\"{}\")] uint {};",
                            texture_kind(uniform.texture),
                            uniform.name
                        ),
                    ),
                    ty => line(1, &format!("{} {};", ty.slang(), uniform.name)),
                }
            }
            line(0, "};");
            line(0, "");
        }
        line(0, "struct Push {");
        if parameters {
            line(1, "Parameters* parameters;");
        }
        for (name, _) in &images {
            line(1, &format!("uint {name};"));
        }
        line(1, "uint target;");
        line(1, "uint2 size;");
        if timed {
            line(1, "float time;");
        }
        line(0, "};");
        line(0, "");
        line(0, "[[vk::push_constant]] ConstantBuffer<Push> push;");
        line(0, "");
        line(0, "[shader(\"compute\")]");
        line(0, "[numthreads(8, 8, 1)]");
        line(
            0,
            &format!("void {PIXEL_ENTRY}(uint3 id: SV_DispatchThreadID) {{"),
        );
        line(1, "int2 pixel = int2(id.xy);");
        line(1, "if (rikka_effect_outside(id.xy, push.size)) {");
        line(2, "return;");
        line(1, "}");
        line(1, "float2 uv = rikka_effect_uv(pixel, push.size);");
        if parameters {
            line(1, "Parameters parameters = *push.parameters;");
        }
        for text in &lines {
            line(1, text);
        }
        line(
            1,
            &format!("rikka_effect_store(push.target, pixel, {color});"),
        );
        line(0, "}");

        let (path, tag) = match &root {
            Root::Slot((path, output)) => (path, output.as_str()),
            Root::Input(path, port) => (path, port.as_str()),
        };
        self.steps.push(Step::Pixel(PixelStep {
            key: hash(path, tag) ^ 0x9e37_79b9_7f4a_7c15,
            source,
            images,
            sockets: uniforms.list.iter().map(Socket::of).collect(),
            timed,
        }));
        let step = self.steps.len() - 1;
        self.pixels.insert(root, step);
        step
    }
}

fn widened(constant: &Constant) -> Constant {
    let at = |index: usize| constant.component(index);
    match constant.width() {
        1 => Constant::vec4(at(0), at(0), at(0), 1.0),
        2 => Constant::vec4(at(0), at(1), 0.0, 1.0),
        3 => Constant::vec4(at(0), at(1), at(2), 1.0),
        _ => *constant,
    }
}

pub(crate) fn diagnose(_library: &Library, flat: &Flat<'_>) -> Vec<GraphError> {
    let mut errors = Vec::new();
    let Some((path, _)) = flat.output() else {
        return errors;
    };
    let mut pixels = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut pending: Vec<(Path, bool)> = vec![(path.clone(), true)];
    while let Some((path, image)) = pending.pop() {
        let node = &flat.nodes[&path];
        let filter = matches!(producer(flat, &path), Some(Producer::Filter(..)))
            || *node.kind == NodeKind::Output;
        if matches!(node.kind, NodeKind::Layer(_)) || !seen.insert((path.clone(), image)) {
            continue;
        }
        if filter {
            errors.extend(world::uniform_errors(flat, node));
        } else {
            pixels.insert(path.clone());
        }
        for (port, from, _) in node.sources() {
            if !filter || takes_image(flat, node, port) {
                pending.push((from.clone(), true));
            }
        }
    }
    errors.extend(flat.site_errors(&pixels, Site::Pixel));
    errors
}
