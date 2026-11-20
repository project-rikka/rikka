use std::collections::BTreeMap;

use rikka_slang::TextureKind;

use crate::context::{Site, expand, input_code};
use crate::flat::{Flat, FlatNode, Path, Slot, Source};
use crate::graph::{Scope, SocketRef};
use crate::library::Behavior;
use crate::node::{BlendMode, Hint, Interpolation, NodeKind, Noise};
use crate::parameter::float;
use crate::ports::swizzle;
use crate::types::{Coercion, Constant, ValueType};

pub(crate) const NO_TEXTURE: &str = "rikka_graph_no_texture()";
pub(crate) const NO_SHADE: &str = "rikka_shade(float3(0.0), 1.0)";
pub(crate) fn zero(ty: ValueType) -> String {
    match ty {
        ValueType::Float => "0.0".to_owned(),
        ValueType::Texture => NO_TEXTURE.to_owned(),
        ValueType::Shader => NO_SHADE.to_owned(),
        other => format!("{}(0.0)", other.slang()),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Constant(Constant),
    Code(String, ValueType),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Uniform {
    pub at: SocketRef,
    pub name: String,
    pub ty: ValueType,
    pub texture: TextureKind,
    pub attributes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Socket {
    pub at: SocketRef,
    pub name: String,
    pub ty: ValueType,
    pub texture: Option<TextureKind>,
}

impl Socket {
    pub(crate) fn of(uniform: &Uniform) -> Self {
        Self {
            at: uniform.at.clone(),
            name: uniform.name.clone(),
            ty: uniform.ty,
            texture: (uniform.ty == ValueType::Texture).then_some(uniform.texture),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Uniforms {
    pub list: Vec<Uniform>,
}

impl Uniforms {
    fn name(flat: &Flat<'_>, at: &SocketRef) -> String {
        let port: String = at
            .port
            .chars()
            .map(|letter| match letter.is_ascii_alphanumeric() {
                true => letter.to_ascii_lowercase(),
                false => '_',
            })
            .collect();
        match &at.scope {
            Scope::Main => format!("u_n{}_{port}", at.node),
            Scope::Function(name) => format!(
                "u_f{}_n{}_{port}",
                flat.graph.function_index(name).unwrap_or(0),
                at.node
            ),
        }
    }

    pub(crate) fn declare(
        &mut self,
        flat: &Flat<'_>,
        at: &SocketRef,
        ty: ValueType,
        texture: TextureKind,
    ) -> &mut Uniform {
        let found = self.list.iter().position(|uniform| uniform.at == *at);
        let index = found.unwrap_or_else(|| {
            self.list.push(Uniform {
                at: at.clone(),
                name: Self::name(flat, at),
                ty,
                texture,
                attributes: Vec::new(),
            });
            self.list.len() - 1
        });
        &mut self.list[index]
    }
}

pub(crate) struct Emitter<'a, 'g> {
    pub flat: &'a Flat<'g>,
    pub site: Site,
    uniforms: &'a mut Uniforms,
    values: BTreeMap<Slot, Value>,
    local: String,
    pub lines: Vec<String>,
    pub copied: BTreeMap<String, &'static str>,
    pub timed: bool,
}

impl<'a, 'g> Emitter<'a, 'g> {
    pub(crate) fn new(flat: &'a Flat<'g>, site: Site, uniforms: &'a mut Uniforms) -> Self {
        Self {
            flat,
            site,
            uniforms,
            values: BTreeMap::new(),
            local: String::new(),
            lines: Vec::new(),
            copied: BTreeMap::new(),
            timed: false,
        }
    }

    pub(crate) fn annotate(&mut self, at: &SocketRef, attribute: String) {
        if let Some(uniform) = self.uniforms.list.iter_mut().find(|known| known.at == *at)
            && !uniform.attributes.contains(&attribute)
        {
            uniform.attributes.push(attribute);
        }
    }

    pub(crate) fn seed(&mut self, slot: Slot, value: Value) {
        self.values.insert(slot, value);
    }

    pub(crate) fn found(&self, slot: &Slot) -> Option<&Value> {
        self.values.get(slot)
    }

    pub(crate) fn values(&self) -> &BTreeMap<Slot, Value> {
        &self.values
    }

    pub(crate) fn run(&mut self, order: &[Path]) {
        for path in order {
            let node = &self.flat.nodes[path];
            self.node(path, node);
        }
    }

    fn field(&mut self, name: &str, ty: &'static str) -> String {
        match self.site {
            Site::Light | Site::Indirect | Site::Post => {
                self.copied.insert(name.to_owned(), ty);
                format!("state.surface.model.{name}")
            }
            Site::Pixel => format!("parameters.{name}"),
            _ => format!("this.{name}"),
        }
    }

    fn texture_kind(&self, node: &FlatNode<'_>, port: &str) -> TextureKind {
        match node.kind {
            NodeKind::Texture(kind) => *kind,
            _ => self
                .flat
                .types(node)
                .ports
                .input(port)
                .and_then(|port| match port.hint {
                    Hint::Texture(kind) => Some(kind),
                    _ => None,
                })
                .unwrap_or(TextureKind::Data),
        }
    }

    pub(crate) fn target(&self, node: &FlatNode<'_>, port: &str) -> ValueType {
        let types = self.flat.types(node);
        match node.kind {
            NodeKind::FunctionInput(_) => types.outputs.get(port),
            _ => types.inputs.get(port),
        }
        .copied()
        .expect("a checked input has a type")
    }

    pub(crate) fn source(&mut self, node: &FlatNode<'_>, port: &str, source: &Source) -> Value {
        let target = self.target(node, port);
        match source {
            Source::Link(from, output) => self
                .values
                .get(&(from.clone(), output.clone()))
                .cloned()
                .unwrap_or_else(|| panic!("{from:?}.{output} is emitted before its users")),
            Source::Constant(constant) => Value::Constant(*constant),
            Source::Socket(at, held) => {
                let (ty, slang) = match held.number() {
                    Some(_) => (target, target.slang()),
                    None => (
                        ValueType::Texture,
                        match self.site {
                            Site::Pixel => "uint",
                            _ => "RikkaTexture",
                        },
                    ),
                };
                let kind = self.texture_kind(node, port);
                let name = self.uniforms.declare(self.flat, at, ty, kind).name.clone();
                Value::Code(self.field(&name, slang), ty)
            }
            Source::Context(input) => {
                self.timed |= *input == crate::node::Input::Time;
                Value::Code(
                    input_code(*input, self.site).expect("a checked input is available"),
                    input.ty(),
                )
            }
            Source::Empty => match target {
                ValueType::Texture => Value::Code(
                    match self.site {
                        Site::Pixel => "RIKKA_NONE".to_owned(),
                        _ => NO_TEXTURE.to_owned(),
                    },
                    ValueType::Texture,
                ),
                ValueType::Shader => Value::Code(NO_SHADE.to_owned(), ValueType::Shader),
                other => unreachable!("an empty {other} input is never emitted"),
            },
            Source::Links(_) | Source::Missing => {
                unreachable!("a checked graph connects every required input")
            }
        }
    }

    pub(crate) fn argument(&mut self, node: &FlatNode<'_>, port: &str) -> Value {
        let target = self.target(node, port);
        let source = node.input(port).clone();
        let value = self.source(node, port, &source);
        coerce(value, target)
    }

    pub(crate) fn text(&mut self, node: &FlatNode<'_>, port: &str) -> String {
        let value = self.argument(node, port);
        text(&value)
    }

    pub(crate) fn set(&mut self, path: &Path, port: &str, value: Value) {
        self.values.insert((path.clone(), port.to_owned()), value);
    }

    pub(crate) fn fresh(&mut self, path: &Path, node: &FlatNode<'_>) -> String {
        local_name(path, &node.kind.title(self.flat.library))
    }

    pub(crate) fn statement(&mut self, path: &Path, port: &str, ty: ValueType, code: String) {
        let name = self.local.clone();
        self.lines.push(format!("{} {name} = {code};", ty.slang()));
        self.set(path, port, Value::Code(name, ty));
    }

    pub(crate) fn begin(&mut self, path: &Path, node: &FlatNode<'_>) {
        self.local = self.fresh(path, node);
    }

    pub(crate) fn node(&mut self, path: &Path, node: &FlatNode<'g>) {
        let types = self.flat.types(node);
        self.begin(path, node);
        let output = |port: &str| types.outputs[port];
        match node.kind {
            NodeKind::Constant(constant) => self.set(path, "value", Value::Constant(*constant)),
            NodeKind::Parameter(name) => {
                let parameter = self
                    .flat
                    .graph
                    .parameter(name)
                    .expect("a checked parameter exists");
                let field = self.field(name, parameter.slang_type());
                self.set(
                    path,
                    "value",
                    Value::Code(parameter.read(&field), parameter.ty()),
                );
            }
            NodeKind::Input(input) => {
                self.timed |= *input == crate::node::Input::Time;
                let code = input_code(*input, self.site).expect("a checked input is available");
                self.statement(path, "value", input.ty(), code);
            }
            NodeKind::FunctionInput(_) | NodeKind::FunctionOutput(_) => {
                let value = match node.input("value") {
                    Source::Empty | Source::Missing => match self.target(node, "value") {
                        ValueType::Texture => {
                            Value::Code(NO_TEXTURE.to_owned(), ValueType::Texture)
                        }
                        _ => Value::Code(NO_SHADE.to_owned(), ValueType::Shader),
                    },
                    _ => self.argument(node, "value"),
                };
                self.set(path, "value", value);
            }
            NodeKind::Op(op) => {
                let info = op.info();
                let args: Vec<Value> = info
                    .inputs
                    .iter()
                    .map(|port| self.argument(node, port.name))
                    .collect();
                let constants: Option<Vec<Constant>> = args
                    .iter()
                    .map(|arg| match arg {
                        Value::Constant(constant) => Some(*constant),
                        Value::Code(..) => None,
                    })
                    .collect();
                let first = info.outputs[0].name;
                let width = types.inputs[info.inputs[0].name]
                    .width()
                    .expect("ops read vectors");
                if let Some(folded) = constants.and_then(|constants| op.fold(&constants, width)) {
                    for (port, constant) in info.outputs.iter().zip(folded) {
                        self.set(path, port.name, Value::Constant(constant));
                    }
                    return;
                }
                let mut texts: Vec<String> = args.iter().map(text).collect();
                if op.reads_point() {
                    texts[0] = point(&args[0]);
                }
                let code = match (op, self.site.pixel()) {
                    (crate::ops::Op::NormalMap, Some(pixel)) => format!(
                        "rikka_graph_normal_map({pixel}.geometric_normal, {pixel}.tangent, {}, {})",
                        texts[0], texts[1]
                    ),
                    _ => op.expression(&texts, output(first)),
                };
                match op.packed() {
                    Some((ty, parts)) => {
                        let packed = self.local.clone();
                        self.lines
                            .push(format!("{} {packed} = {code};", ty.slang()));
                        for (port, swizzle) in parts {
                            self.set(
                                path,
                                port,
                                Value::Code(format!("{packed}{swizzle}"), output(port)),
                            );
                        }
                    }
                    None => self.statement(path, first, output(first), code),
                }
            }
            NodeKind::Swizzle(mask) => {
                let value = self.argument(node, "value");
                let chosen = swizzle(mask).expect("a checked swizzle");
                let ty = output("value");
                match value {
                    Value::Constant(constant) => {
                        let picked: Vec<f32> = chosen
                            .iter()
                            .map(|index| constant.component(*index))
                            .collect();
                        self.set(
                            path,
                            "value",
                            Value::Constant(Constant::from_slice(&picked).expect("a swizzle")),
                        );
                    }
                    Value::Code(code, from) => {
                        let mask: String =
                            chosen.iter().map(|index| b"xyzw"[*index] as char).collect();
                        let code = if from == ValueType::Float {
                            format!("{}({code})", ty.slang())
                        } else {
                            format!("{code}.{mask}")
                        };
                        self.statement(path, "value", ty, code);
                    }
                }
            }
            NodeKind::Texture(kind) => {
                let texture = self.text(node, "texture");
                let at = match kind {
                    TextureKind::Ramp => format!("float2({}, 0.5)", self.text(node, "position")),
                    _ => self.text(node, "uv"),
                };
                let sampled = match self.site {
                    Site::Pixel => format!("rikka_effect_sample({texture}, {at})"),
                    Site::Vertex | Site::Hull => format!("{texture}.sampled_level({at}, 0.0)"),
                    _ => format!("{texture}.sampled({at})"),
                };
                match kind {
                    TextureKind::Normal => {
                        let code = if self.site.is_material() {
                            format!(
                                "({texture}.bound() ? rikka_unpack_normal({sampled}) : float3(0.0, 0.0, 1.0))"
                            )
                        } else {
                            format!("rikka_unpack_normal({sampled})")
                        };
                        self.statement(path, "normal", ValueType::Float3, code);
                    }
                    _ => self.pack(path, format!("float4 {} = {sampled};", self.local)),
                }
            }
            NodeKind::Fbm(noise) => {
                let position = point(&self.argument(node, "position"));
                let [scale, octaves, lacunarity, gain] =
                    ["scale", "octaves", "lacunarity", "gain"].map(|port| self.text(node, port));
                let function = match noise {
                    Noise::Value => "rikka_graph_fbm_value",
                    Noise::Gradient => "rikka_graph_fbm_gradient",
                };
                self.statement(
                    path,
                    "value",
                    ValueType::Float,
                    format!("{function}({position} * {scale}, {octaves}, {lacunarity}, {gain})"),
                );
            }
            NodeKind::Blend(mode) => {
                let [base, blend, opacity] =
                    ["base", "blend", "opacity"].map(|port| self.text(node, port));
                let mixed = blend_expression(*mode, &base, &blend);
                self.statement(
                    path,
                    "value",
                    ValueType::Float3,
                    format!("lerp({base}, {mixed}, {opacity})"),
                );
            }
            NodeKind::ColorRamp(ramp) => {
                let mut stops = ramp.stops.clone();
                stops.sort_by(|a, b| a.position.total_cmp(&b.position));
                let points: Vec<(f32, Constant)> = stops
                    .iter()
                    .map(|stop| {
                        let [r, g, b, a] = stop.color;
                        (stop.position, Constant::vec4(r, g, b, a))
                    })
                    .collect();
                self.piecewise(
                    path,
                    node,
                    "position",
                    &points,
                    ramp.interpolation,
                    ValueType::Float4,
                );
            }
            NodeKind::Curve(curve) => {
                let mut sorted = curve.points.clone();
                sorted.sort_by(|a, b| a[0].total_cmp(&b[0]));
                let points: Vec<(f32, Constant)> = sorted
                    .iter()
                    .map(|[x, y]| (*x, Constant::scalar(*y)))
                    .collect();
                self.piecewise(
                    path,
                    node,
                    "value",
                    &points,
                    curve.interpolation,
                    ValueType::Float,
                );
            }
            NodeKind::Emission => {
                let [color, strength] = ["color", "strength"].map(|port| self.text(node, port));
                self.statement(
                    path,
                    "shader",
                    ValueType::Shader,
                    format!("rikka_shade({color} * {strength}, 1.0)"),
                );
            }
            NodeKind::Transparent => self.statement(
                path,
                "shader",
                ValueType::Shader,
                "rikka_shade(float3(0.0), 0.0)".to_owned(),
            ),
            NodeKind::MixShader => {
                let [a, b, factor] = ["a", "b", "factor"].map(|port| self.text(node, port));
                self.statement(
                    path,
                    "shader",
                    ValueType::Shader,
                    format!("rikka_shade_mix({a}, {b}, {factor})"),
                );
            }
            NodeKind::AddShader => {
                let [a, b] = ["a", "b"].map(|port| self.text(node, port));
                self.statement(
                    path,
                    "shader",
                    ValueType::Shader,
                    format!("rikka_shade_add({a}, {b})"),
                );
            }
            NodeKind::ShaderToRgb => {
                let shader = self.text(node, "shader");
                let packed = self.local.clone();
                self.lines
                    .push(format!("float4 {packed} = rikka_shade_straight({shader});"));
                self.set(
                    path,
                    "color",
                    Value::Code(format!("{packed}.rgb"), ValueType::Float3),
                );
                self.set(
                    path,
                    "alpha",
                    Value::Code(format!("{packed}.a"), ValueType::Float),
                );
            }
            NodeKind::Extern(_) => {
                let spec = self.flat.spec(node).expect("a checked node kind");
                let Behavior::Function(function) = &spec.behavior else {
                    unreachable!("only function nodes are emitted as values");
                };
                let arguments: BTreeMap<&str, String> = spec
                    .inputs
                    .iter()
                    .map(|port| (port.name, self.text(node, port.name)))
                    .collect();
                let code = expand(function.code, self.site, &|name| {
                    arguments.get(name).cloned()
                });
                if function.outputs.is_empty() {
                    let port = spec.outputs[0].name;
                    self.statement(path, port, output(port), code);
                } else {
                    let packed = self.local.clone();
                    self.lines
                        .push(format!("{} {packed} = {code};", function.ty));
                    for (port, member) in &function.outputs {
                        self.set(
                            path,
                            port,
                            Value::Code(format!("{packed}{member}"), output(port)),
                        );
                    }
                }
            }
            NodeKind::Lighting
            | NodeKind::Layer(_)
            | NodeKind::Sample
            | NodeKind::Call(_)
            | NodeKind::Output => {}
        }
    }

    pub(crate) fn pack(&mut self, path: &Path, declaration: String) {
        let packed = self.local.clone();
        self.lines.push(declaration);
        for (port, swizzle, ty) in [
            ("rgba", "", ValueType::Float4),
            ("rgb", ".rgb", ValueType::Float3),
            ("a", ".a", ValueType::Float),
        ] {
            self.set(path, port, Value::Code(format!("{packed}{swizzle}"), ty));
        }
    }

    pub(crate) fn local(&self) -> &str {
        &self.local
    }

    fn piecewise(
        &mut self,
        path: &Path,
        node: &FlatNode<'_>,
        port: &str,
        points: &[(f32, Constant)],
        interpolation: Interpolation,
        ty: ValueType,
    ) {
        if let [(_, only)] = points {
            self.set(path, "value", Value::Constant(*only));
            return;
        }
        let at = self.argument(node, port);
        if let Value::Constant(constant) = at {
            let value = sample(points, constant.component(0), interpolation, ty);
            self.set(path, "value", Value::Constant(value));
            return;
        }
        let t = text(&at);
        let name = self.local.clone();
        self.lines.push(format!(
            "{} {name} = {};",
            ty.slang(),
            literal(&points[0].1)
        ));
        for pair in points.windows(2) {
            let factor = factor_code(&t, pair[0].0, pair[1].0, interpolation);
            self.lines.push(format!(
                "{name} = lerp({name}, {}, {factor});",
                literal(&pair[1].1)
            ));
        }
        self.set(path, "value", Value::Code(name, ty));
    }
}

pub(crate) fn sample(
    points: &[(f32, Constant)],
    t: f32,
    interpolation: Interpolation,
    ty: ValueType,
) -> Constant {
    let mut value = points[0].1;
    for pair in points.windows(2) {
        let factor = factor_value(t, pair[0].0, pair[1].0, interpolation);
        let values: Vec<f32> = (0..ty.width().expect("a vector"))
            .map(|i| {
                let from = value.component(i);
                from + (pair[1].1.component(i) - from) * factor
            })
            .collect();
        value = Constant::from_slice(&values).expect("a ramp value");
    }
    value
}

fn factor_value(t: f32, from: f32, to: f32, interpolation: Interpolation) -> f32 {
    let step = if t >= to { 1.0 } else { 0.0 };
    if to <= from || interpolation == Interpolation::Constant {
        return step;
    }
    let linear = ((t - from) / (to - from)).clamp(0.0, 1.0);
    match interpolation {
        Interpolation::Smooth => linear * linear * (3.0 - 2.0 * linear),
        _ => linear,
    }
}

fn factor_code(t: &str, from: f32, to: f32, interpolation: Interpolation) -> String {
    if to <= from || interpolation == Interpolation::Constant {
        return format!("step({}, {t})", float(to));
    }
    match interpolation {
        Interpolation::Smooth => format!("smoothstep({}, {}, {t})", float(from), float(to)),
        _ => format!(
            "saturate(({t} - {}) * {})",
            float(from),
            float(1.0 / (to - from))
        ),
    }
}

fn blend_expression(mode: BlendMode, base: &str, blend: &str) -> String {
    match mode {
        BlendMode::Normal => blend.to_owned(),
        BlendMode::Multiply => format!("({base} * {blend})"),
        BlendMode::Screen => format!("(1.0 - (1.0 - {base}) * (1.0 - {blend}))"),
        BlendMode::Overlay => format!("rikka_graph_blend_overlay({base}, {blend})"),
        BlendMode::SoftLight => format!("rikka_graph_blend_soft_light({base}, {blend})"),
        BlendMode::Add => format!("({base} + {blend})"),
        BlendMode::Subtract => format!("({base} - {blend})"),
        BlendMode::Difference => format!("abs({base} - {blend})"),
        BlendMode::Darken => format!("min({base}, {blend})"),
        BlendMode::Lighten => format!("max({base}, {blend})"),
        BlendMode::Dodge => format!("rikka_graph_blend_dodge({base}, {blend})"),
        BlendMode::Burn => format!("rikka_graph_blend_burn({base}, {blend})"),
        BlendMode::Divide => format!("rikka_graph_blend_divide({base}, {blend})"),
        BlendMode::LinearLight => format!("({base} + 2.0 * {blend} - 1.0)"),
        BlendMode::Exclusion => format!("({base} + {blend} - 2.0 * {base} * {blend})"),
        BlendMode::Hue => format!("rikka_graph_blend_hue({base}, {blend})"),
        BlendMode::Saturation => format!("rikka_graph_blend_saturation({base}, {blend})"),
        BlendMode::Color => format!("rikka_graph_blend_color({base}, {blend})"),
        BlendMode::Value => format!("rikka_graph_blend_value({base}, {blend})"),
    }
}

pub(crate) fn local_name(path: &Path, title: &str) -> String {
    let mut name = String::new();
    for word in title
        .split(|letter: char| !letter.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
    {
        name.push_str(&word.to_ascii_lowercase());
        name.push('_');
    }
    if name.starts_with(|letter: char| letter.is_ascii_digit()) || name.is_empty() {
        name.insert_str(0, "node_");
    }
    let ids: Vec<String> = path.iter().map(ToString::to_string).collect();
    name.push_str(&ids.join("_"));
    name
}

fn point(value: &Value) -> String {
    let code = text(value);
    match value_type(value) {
        ValueType::Float => format!("float3({code}, 0.0, 0.0)"),
        ValueType::Float2 => format!("float3({code}, 0.0)"),
        ValueType::Float4 => format!("{code}.xyz"),
        _ => code,
    }
}

pub(crate) fn value_type(value: &Value) -> ValueType {
    match value {
        Value::Constant(constant) => constant.ty(),
        Value::Code(_, ty) => *ty,
    }
}

pub(crate) fn literal(constant: &Constant) -> String {
    let values: Vec<String> = constant
        .values()
        .iter()
        .map(|value| float(*value))
        .collect();
    match constant.width() {
        1 => values[0].clone(),
        width => format!("float{width}({})", values.join(", ")),
    }
}

pub(crate) fn text(value: &Value) -> String {
    match value {
        Value::Constant(constant) => literal(constant),
        Value::Code(code, _) => code.clone(),
    }
}

pub(crate) fn coerce(value: Value, target: ValueType) -> Value {
    match value {
        Value::Constant(constant) => match target.width() {
            Some(width) => Value::Constant(constant.resized(width)),
            None => Value::Constant(constant),
        },
        Value::Code(code, from) => match from.coerce_to(target) {
            Ok(Coercion::Splat(_)) => Value::Code(format!("{}({code})", target.slang()), target),
            Ok(Coercion::Truncate(width)) => {
                Value::Code(format!("{code}.{}", &"xyzw"[..width]), target)
            }
            Ok(Coercion::Same | Coercion::Load(_) | Coercion::Store) | Err(_) => {
                Value::Code(code, from)
            }
        },
    }
}
