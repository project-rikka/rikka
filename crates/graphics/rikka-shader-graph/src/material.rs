use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use rikka_slang::{Role, TextureKind};

use crate::context::{Site, Taint};
use crate::emit::{Emitter, NO_SHADE, Socket, Uniforms, Value, zero};
use crate::error::{Error, GraphError};
use crate::flat::{Flat, FlatNode, Path, Slot, Source};
use crate::gate::{Condition, Control, Features, Gating, Imports};
use crate::graph::{Graph, GraphKind};
use crate::library::{ClosureCode, Field, Library};
use crate::node::NodeKind;
use crate::parameter::float;
use crate::ports::OUTLINE_PORTS;
use crate::staging::{Closure, Side, closure, stage};
use crate::types::{SocketValue, ValueType};

pub(crate) const RESERVED: &[&str] = &["gloss", "see_through"];
pub(crate) const SOCKET_PREFIX: &str = "u_";

const FEATURE_MARK: u32 = 31;

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialSource {
    pub name: String,
    pub source: String,
    pub sockets: Vec<Socket>,
    pub outlined: bool,
    pub displaces: bool,
}

impl Graph {
    pub fn material(&self, library: &Library, name: &str) -> Result<MaterialSource, Error> {
        let report = self.validate(library);
        if !report.is_valid() || self.kind() != GraphKind::Material {
            return Err(Error(report.errors));
        }
        let flat = Flat::new(self, library, &report);
        Ok(generate(&flat, name))
    }
}

fn linked(source: &Source) -> bool {
    matches!(source, Source::Link(..))
}

fn cone_of(flat: &Flat<'_>, output: &FlatNode<'_>, ports: &[&str]) -> BTreeSet<Path> {
    flat.cone(ports.iter().filter_map(|port| match output.input(port) {
        Source::Link(from, _) => Some(from.clone()),
        _ => None,
    }))
}

pub(crate) fn diagnose(_graph: &Graph, _library: &Library, flat: &Flat<'_>) -> Vec<GraphError> {
    let mut errors = Vec::new();
    let Some((_, output)) = flat.output() else {
        return errors;
    };
    stage(flat, output, false, &["surface", "alpha"], &mut errors);
    if linked(output.input("outline")) {
        stage(flat, output, true, &["outline"], &mut errors);
        errors.extend(flat.site_errors(&cone_of(flat, output, &OUTLINE_PORTS[1..]), Site::Hull));
    }
    errors.extend(flat.site_errors(&cone_of(flat, output, &["displacement"]), Site::Vertex));
    errors
}

#[derive(Default)]
struct SideCode {
    surface: Vec<String>,
    begins: Vec<Vec<String>>,
    illuminates: Vec<bool>,
    combines: Vec<Vec<String>>,
    direct: Vec<Vec<String>>,
    transmit: Vec<Vec<String>>,
    indirect: Vec<Vec<String>>,
    finish: Vec<String>,
    scatter: Vec<String>,
}

struct Member {
    ty: String,
    name: String,
    zero: String,
}

fn add(members: &mut Vec<Member>, member: Member) {
    if !members.iter().any(|known| known.name == member.name) {
        members.push(member);
    }
}

#[derive(Default)]
struct Carried {
    produced: BTreeMap<Slot, (usize, Value)>,
    fields: BTreeMap<Slot, String>,
    stores: BTreeMap<usize, Vec<String>>,
}

impl Imports for Carried {
    fn import(&mut self, emitter: &mut Emitter<'_, '_>, node: &FlatNode<'_>) {
        let ports: Vec<String> = node.inputs.iter().map(|(port, _)| port.clone()).collect();
        for port in ports {
            self.import_port(emitter, node, &port);
        }
    }

    fn import_port(&mut self, emitter: &mut Emitter<'_, '_>, node: &FlatNode<'_>, port: &str) {
        let Source::Link(from, output) = node.input(port) else {
            return;
        };
        let slot = (from.clone(), output.clone());
        if emitter.found(&slot).is_some() {
            return;
        }
        let Some((level, value)) = self.produced.get(&slot) else {
            return;
        };
        let value = match value {
            Value::Constant(_) => value.clone(),
            Value::Code(expression, ty) => {
                let field = match self.fields.get(&slot) {
                    Some(field) => field.clone(),
                    None => {
                        let field = carried_name(&slot);
                        let store = match level {
                            0 => format!("surface.model.{field} = {expression};"),
                            _ => format!("state.{field} = {expression};"),
                        };
                        self.stores.entry(*level).or_default().push(store);
                        self.fields.insert(slot.clone(), field.clone());
                        field
                    }
                };
                Value::Code(
                    match level {
                        0 => format!("state.surface.model.{field}"),
                        _ => format!("state.{field}"),
                    },
                    *ty,
                )
            }
        };
        emitter.seed(slot, value);
    }
}

impl Carried {
    fn record(&mut self, emitter: &Emitter<'_, '_>, own: BTreeSet<Slot>, level: usize) {
        for slot in own {
            if let Some(value) = emitter.found(&slot) {
                self.produced.insert(slot, (level, value.clone()));
            }
        }
    }
}

fn carried_name(slot: &Slot) -> String {
    let port: String = slot
        .1
        .chars()
        .map(|letter| match letter.is_ascii_alphanumeric() {
            true => letter.to_ascii_lowercase(),
            false => '_',
        })
        .collect();
    format!("v_{}_{port}", id_of(&slot.0))
}

struct Generator<'f, 'g> {
    flat: &'f Flat<'g>,
    data: Vec<Member>,
    state: Vec<Member>,
    copied: BTreeMap<String, &'static str>,
    forced: BTreeSet<u32>,
    featured: bool,
    routed: Vec<Role>,
    extras: Vec<String>,
    scatters: bool,
    features: Features,
}

fn id_of(path: &Path) -> String {
    path.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("_")
}

fn hook(model: &ClosureCode, hook: &str) -> String {
    format!("rikka_closure_{hook}<{}>", model.model)
}

fn finish(emitter: &mut Emitter<'_, '_>, side: &Side, output: &FlatNode<'_>) -> Vec<String> {
    let shade = emitter.text(output, side.roots[0].0);
    let alpha = match side.roots.get(1) {
        Some((port, _)) => format!("lit.a * {}", emitter.text(output, port)),
        None => "lit.a".to_owned(),
    };
    vec![
        format!("float4 lit = rikka_shade_straight({shade});"),
        format!("return float4(lit.rgb, rikka_coverage(context, {alpha}));"),
    ]
}

fn wrapped(condition: Condition, lines: Vec<String>) -> Vec<String> {
    match condition {
        Condition::Never => Vec::new(),
        Condition::Always => lines,
        Condition::When(runs) => std::iter::once(format!("if ({runs}) {{"))
            .chain(lines.into_iter().map(|line| format!("    {line}")))
            .chain(std::iter::once("}".to_owned()))
            .collect(),
    }
}

fn live(condition: Condition) -> String {
    match condition {
        Condition::Never => "false".to_owned(),
        Condition::Always => "true".to_owned(),
        Condition::When(runs) => runs,
    }
}

fn main_closure(flat: &Flat<'_>, side: &Side, output: &FlatNode<'_>) -> Option<Path> {
    let mut pending: Vec<Source> = vec![output.input("surface").clone()];
    while let Some(source) = pending.pop() {
        let Source::Link(path, _) = source else {
            continue;
        };
        let node = &flat.nodes[&path];
        match node.kind {
            NodeKind::FunctionInput(_) | NodeKind::FunctionOutput(_) => {
                pending.push(node.input("value").clone());
            }
            NodeKind::MixShader | NodeKind::AddShader => {
                pending.push(node.input("b").clone());
                pending.push(node.input("a").clone());
            }
            _ => {
                if let Some(Closure::Model(_)) = closure(flat, node)
                    && side.stages.get(&path) == Some(&0)
                {
                    return Some(path);
                }
            }
        }
    }
    None
}

struct Staged<'a> {
    path: &'a Path,
    model: &'a ClosureCode,
}

fn begin(
    gating: &Gating<'_, '_>,
    emitter: &mut Emitter<'_, '_>,
    carried: &mut Carried,
    shared: &[Staged<'_>],
    lit: &[Staged<'_>],
    level: usize,
) -> Vec<String> {
    let pixel = match level {
        0 => "pixel",
        _ => "state.pixel",
    };
    let data = |path: &Path| match level {
        0 => format!("surface.model.c_{}", id_of(path)),
        _ => format!("c_{}", id_of(path)),
    };
    let mut lines = Vec::new();
    for staged in shared {
        let prepare = format!(
            "state.s_{} = {}({pixel}, {}, context);",
            id_of(staged.path),
            hook(staged.model, "prepare"),
            data(staged.path)
        );
        let condition = gating.condition(emitter, carried, &gating.guard(staged.path));
        lines.extend(wrapped(condition, vec![prepare]));
    }
    let mut models: Vec<(&str, Vec<&Path>)> = Vec::new();
    for staged in lit {
        match models
            .iter_mut()
            .find(|(model, _)| *model == staged.model.model)
        {
            Some((_, paths)) => paths.push(staged.path),
            None => models.push((staged.model.model, vec![staged.path])),
        }
    }
    for (model, paths) in models {
        let lives: Vec<String> = paths
            .iter()
            .map(|path| live(gating.condition(emitter, carried, &gating.runs(path))))
            .collect();
        if let ([path], [alive]) = (paths.as_slice(), lives.as_slice()) {
            lines.push(format!(
                "state.r_{} = rikka_closure_lit<{model}>({pixel}, {}, {alive}, context);",
                id_of(path),
                data(path)
            ));
            continue;
        }
        let name = format!("lit_{}", id_of(paths[0]));
        let count = paths.len();
        lines.push(format!(
            "RikkaSurfaceData<{model}.Data> {name}_data[{count}];"
        ));
        lines.push(format!("bool {name}_live[{count}];"));
        lines.push(format!("RikkaShade {name}_shade[{count}];"));
        for (index, (path, alive)) in paths.iter().zip(&lives).enumerate() {
            lines.push(format!("{name}_data[{index}] = {};", data(path)));
            lines.push(format!("{name}_live[{index}] = {alive};"));
        }
        lines.extend([
            "[loop]".to_owned(),
            format!("for (uint index = 0u; index < {count}u; index++) {{"),
            format!(
                "    {name}_shade[index] = rikka_closure_lit<{model}>({pixel}, {name}_data[index], {name}_live[index], context);"
            ),
            "}".to_owned(),
        ]);
        for (index, path) in paths.iter().enumerate() {
            lines.push(format!("state.r_{} = {name}_shade[{index}];", id_of(path)));
        }
    }
    lines
}

impl<'g> Generator<'_, 'g> {
    fn side(&mut self, side: &Side, uniforms: &mut Uniforms) -> SideCode {
        let flat = self.flat;
        let output = flat.output().expect("a checked graph has an output").1;
        let main = match side.hull {
            true => None,
            false => main_closure(flat, side, output),
        };
        let gating = Gating::new(flat, side, main.as_ref(), &mut self.features);
        let mut code = SideCode::default();
        let mut carried = Carried::default();
        let closures: Vec<(&Path, usize)> = side
            .order
            .iter()
            .filter_map(|path| side.stages.get(path).map(|stage| (path, *stage)))
            .collect();
        let separate = |level: usize| {
            closures
                .iter()
                .filter(|(path, stage)| {
                    *stage == level
                        && matches!(closure(flat, &flat.nodes[*path]), Some(Closure::Model(_)))
                })
                .count()
                > 1
        };
        let apart = |path: &Path, level: usize| separate(level) || !gating.runs(path).is_empty();

        for level in 0..=side.loops {
            let site = match level {
                0 => Site::Surface,
                _ => Site::Post,
            };
            let mut emitter = Emitter::new(flat, site, uniforms);
            let mut own: BTreeSet<Slot> = BTreeSet::new();
            let pixel = site.pixel().expect("a pixel site");
            for (path, stage) in &closures {
                if stage + 1 != level {
                    continue;
                }
                let node = &flat.nodes[*path];
                emitter.begin(path, node);
                let id = id_of(path);
                match closure(flat, node).expect("a staged closure") {
                    Closure::Custom => emitter.statement(
                        path,
                        "shader",
                        ValueType::Shader,
                        format!("rikka_shade(state.a_{id}, 1.0)"),
                    ),
                    Closure::Model(_) if apart(path, *stage) => emitter.set(
                        path,
                        "shader",
                        Value::Code(format!("state.r_{id}"), ValueType::Shader),
                    ),
                    Closure::Model(model) => {
                        let shaded = format!(
                            "{}(state.s_{id}, state.pixel, context)",
                            hook(model, "shade")
                        );
                        match gating.condition(&mut emitter, &mut carried, &gating.runs(path)) {
                            Condition::Never => emitter.set(
                                path,
                                "shader",
                                Value::Code(NO_SHADE.to_owned(), ValueType::Shader),
                            ),
                            Condition::Always => {
                                emitter.statement(path, "shader", ValueType::Shader, shaded);
                            }
                            Condition::When(runs) => {
                                let name = emitter.local().to_owned();
                                emitter.lines.extend([
                                    format!("RikkaShade {name} = {NO_SHADE};"),
                                    format!("if ({runs}) {{"),
                                    format!("    {name} = {shaded};"),
                                    "}".to_owned(),
                                ]);
                                emitter.set(path, "shader", Value::Code(name, ValueType::Shader));
                            }
                        }
                    }
                }
                own.insert(((*path).clone(), "shader".to_owned()));
            }
            let list: Vec<Path> = side
                .order
                .iter()
                .filter(|path| {
                    !side.taints.contains_key(*path)
                        && !side.stages.contains_key(*path)
                        && side.levels[*path] == level
                })
                .cloned()
                .collect();
            let emitted: BTreeSet<Path> = list.iter().cloned().collect();
            gating.emit(&mut emitter, &mut carried, &gating.layout(list), 0);
            own.extend(
                emitter
                    .values()
                    .keys()
                    .filter(|slot| emitted.contains(&slot.0))
                    .cloned(),
            );
            let mut shared = Vec::new();
            let mut lit = Vec::new();
            let mut illuminates = false;
            for (path, stage) in &closures {
                if *stage != level {
                    continue;
                }
                let node = &flat.nodes[*path];
                let id = id_of(path);
                carried.import(&mut emitter, node);
                match closure(flat, node).expect("a staged closure") {
                    Closure::Custom => {
                        illuminates = true;
                        add(
                            &mut self.state,
                            Member {
                                ty: "float3".to_owned(),
                                name: format!("a_{id}"),
                                zero: "float3(0.0)".to_owned(),
                            },
                        );
                    }
                    Closure::Model(model) => {
                        let data = format!("c_{id}");
                        let ty = format!("RikkaSurfaceData<{}.Data>", model.model);
                        emitter.lines.push(format!(
                            "{ty} {data} = rikka_surface_data({pixel}, {});",
                            model.data
                        ));
                        for (port, field) in &model.fields {
                            let value = emitter.text(node, port);
                            emitter.lines.push(match field {
                                Field::Albedo => format!("{data}.albedo = {value};"),
                                Field::Alpha => format!("{data}.alpha = {value};"),
                                Field::Normal => format!("{data}.normal = {value};"),
                                Field::Occlusion => format!("{data}.occlusion = {value};"),
                                Field::Emission => format!("{data}.emission = {value};"),
                                Field::Data(name) => format!("{data}.model.{name} = {value};"),
                                Field::Flag(name) => {
                                    format!("{data}.model.{name} = {value} > 0.5;")
                                }
                            });
                        }
                        if let (true, Some(flag)) = (side.hull || level > 0, model.unscattered) {
                            emitter.lines.push(format!("{data}.model.{flag} = true;"));
                        }
                        self.traits(&mut emitter, node, model, side.hull);
                        if level == 0 {
                            add(
                                &mut self.data,
                                Member {
                                    ty,
                                    name: data.clone(),
                                    zero: format!("rikka_surface_data(pixel, {})", model.data),
                                },
                            );
                            emitter
                                .lines
                                .push(format!("surface.model.{data} = {data};"));
                        }
                        let staged = Staged { path, model };
                        match apart(path, level) {
                            true => {
                                add(
                                    &mut self.state,
                                    Member {
                                        ty: "RikkaShade".to_owned(),
                                        name: format!("r_{id}"),
                                        zero: NO_SHADE.to_owned(),
                                    },
                                );
                                lit.push(staged);
                            }
                            false => {
                                illuminates = true;
                                add(
                                    &mut self.state,
                                    Member {
                                        ty: format!("{}.State", model.model),
                                        name: format!("s_{id}"),
                                        zero: String::new(),
                                    },
                                );
                                shared.push(staged);
                            }
                        }
                    }
                }
            }
            if level > 0 {
                let lines = begin(&gating, &mut emitter, &mut carried, &shared, &lit, level);
                emitter.lines.extend(lines);
            }
            if level == 0 && !side.hull {
                let (outer, scatter) = self.outer(side, main.as_ref(), &closures, &mut emitter);
                emitter.lines.extend(outer);
                code.scatter = scatter;
            }
            carried.record(&emitter, own, level);
            if level == side.loops && level > 0 {
                for (port, _) in &side.roots {
                    carried.import_port(&mut emitter, output, port);
                }
                let last = finish(&mut emitter, side, output);
                emitter.lines.extend(last);
            }
            self.copied.extend(std::mem::take(&mut emitter.copied));
            let lines = std::mem::take(&mut emitter.lines);
            drop(emitter);
            match level {
                0 => code.surface = lines,
                level if level == side.loops => code.finish = lines,
                _ => code.combines.push(lines),
            }
            if level == side.loops {
                if level == 0 {
                    let mut emitter = Emitter::new(flat, Site::Post, uniforms);
                    for (port, _) in &side.roots {
                        carried.import_port(&mut emitter, output, port);
                    }
                    code.finish = finish(&mut emitter, side, output);
                    self.copied.extend(std::mem::take(&mut emitter.copied));
                }
                break;
            }
            let mut begun = Vec::new();
            if level == 0 {
                let mut emitter = Emitter::new(flat, Site::Post, uniforms);
                begun = begin(&gating, &mut emitter, &mut carried, &shared, &lit, level);
                self.copied.extend(std::mem::take(&mut emitter.copied));
            }
            code.begins.push(begun);
            code.illuminates.push(illuminates);

            for taint in [Taint::Light, Taint::Indirect] {
                let (site, port) = match taint {
                    Taint::Light => (Site::Light, "light"),
                    Taint::Indirect => (Site::Indirect, "ambient"),
                };
                let mut emitter = Emitter::new(flat, site, uniforms);
                let list: Vec<Path> = side
                    .order
                    .iter()
                    .filter(|path| {
                        side.taints.get(*path) == Some(&taint)
                            && side
                                .uses
                                .get(*path)
                                .is_some_and(|uses| uses.contains(&level))
                    })
                    .cloned()
                    .collect();
                gating.emit(&mut emitter, &mut carried, &gating.layout(list), 0);
                for (path, stage) in &closures {
                    if *stage != level {
                        continue;
                    }
                    let node = &flat.nodes[*path];
                    let id = id_of(path);
                    let lines = match closure(flat, node).expect("a staged closure") {
                        Closure::Custom => {
                            if matches!(node.input(port), Source::Constant(_)) {
                                continue;
                            }
                            carried.import_port(&mut emitter, node, port);
                            let value = emitter.text(node, port);
                            vec![format!("state.a_{id} += {value};")]
                        }
                        Closure::Model(_) if apart(path, level) => continue,
                        Closure::Model(model) => vec![match taint {
                            Taint::Light => format!(
                                "{}(state.s_{id}, light, shadow, context);",
                                hook(model, "direct")
                            ),
                            Taint::Indirect => format!(
                                "{}(state.s_{id}, indirect, context);",
                                hook(model, "indirect")
                            ),
                        }],
                    };
                    let condition =
                        gating.condition(&mut emitter, &mut carried, &gating.runs(path));
                    let guarded = wrapped(condition, lines);
                    emitter.lines.extend(guarded);
                }
                self.copied.extend(std::mem::take(&mut emitter.copied));
                let lines = std::mem::take(&mut emitter.lines);
                match taint {
                    Taint::Light => code.direct.push(lines),
                    Taint::Indirect => code.indirect.push(lines),
                }
            }
            let mut emitter = Emitter::new(flat, Site::Light, uniforms);
            for (path, stage) in &closures {
                if *stage != level || apart(path, level) {
                    continue;
                }
                let Some(Closure::Model(model)) = closure(flat, &flat.nodes[*path]) else {
                    continue;
                };
                let call = format!(
                    "{}(state.s_{}, light, shadow, context);",
                    hook(model, "transmit"),
                    id_of(path)
                );
                let condition = gating.condition(&mut emitter, &mut carried, &gating.runs(path));
                let guarded = wrapped(condition, vec![call]);
                emitter.lines.extend(guarded);
            }
            self.copied.extend(std::mem::take(&mut emitter.copied));
            code.transmit.push(std::mem::take(&mut emitter.lines));
        }
        for (level, lines) in std::mem::take(&mut carried.stores) {
            match level {
                0 => code.surface.extend(lines),
                level => code.combines[level - 1].extend(lines),
            }
        }
        for (slot, field) in &carried.fields {
            let (level, value) = &carried.produced[slot];
            let Value::Code(_, ty) = value else {
                continue;
            };
            let member = Member {
                ty: ty.slang().to_owned(),
                name: field.clone(),
                zero: zero(*ty),
            };
            match level {
                0 => add(&mut self.data, member),
                _ => add(&mut self.state, member),
            }
        }
        code
    }

    fn traits(
        &mut self,
        emitter: &mut Emitter<'_, 'g>,
        node: &FlatNode<'_>,
        model: &ClosureCode,
        hull: bool,
    ) {
        let mut annotate = |port: &str, attribute: String| -> bool {
            match node.input(port) {
                Source::Socket(at, SocketValue::Number(_)) => {
                    emitter.annotate(at, attribute);
                    false
                }
                Source::Constant(constant) => constant.values().iter().any(|value| *value != 0.0),
                Source::Link(..) | Source::Context(_) => true,
                _ => false,
            }
        };
        for (port, bit) in &model.features {
            self.featured = true;
            if annotate(port, format!("Feature({bit})")) {
                self.forced.insert(*bit);
            }
        }
        if hull {
            return;
        }
        for (port, role) in [
            (model.subsurface, Role::Subsurface),
            (model.transmission, Role::Transmission),
        ] {
            let Some(port) = port else {
                continue;
            };
            if role == Role::Subsurface {
                self.scatters = true;
            }
            if self.routed.contains(&role) {
                continue;
            }
            self.routed.push(role);
            if annotate(port, format!("Role(\"{}\")", role.name())) {
                self.extras.push(format!(
                    "[Role(\"{0}\")] [Hint(\"hidden\")] [Default(1.0)] float u_{0};",
                    role.name()
                ));
            }
        }
    }

    fn outer(
        &mut self,
        side: &Side,
        main: Option<&Path>,
        closures: &[(&Path, usize)],
        emitter: &mut Emitter<'_, 'g>,
    ) -> (Vec<String>, Vec<String>) {
        let flat = self.flat;
        let output = flat.output().expect("a checked graph has an output").1;
        let mut lines = Vec::new();
        let mut scatter = Vec::new();
        let mut scattered: Vec<(&str, String)> = Vec::new();
        let mut scattering: BTreeSet<&Path> = BTreeSet::new();
        let mut pending: Vec<(Source, Option<String>)> =
            vec![(output.input("surface").clone(), Some("1.0".to_owned()))];
        let scaled = |weight: &Option<String>, factor: &str| {
            weight
                .as_ref()
                .map(|weight| format!("({weight} * {factor})"))
        };
        while let Some((source, weight)) = pending.pop() {
            let Source::Link(path, _) = source else {
                continue;
            };
            let (path, node) = flat
                .nodes
                .get_key_value(&path)
                .expect("a linked node is flattened");
            match node.kind {
                NodeKind::FunctionInput(_) | NodeKind::FunctionOutput(_) => {
                    pending.push((node.input("value").clone(), weight));
                }
                NodeKind::MixShader => {
                    let factor = (side.level(node.input("factor")) == 0)
                        .then(|| format!("saturate({})", emitter.text(node, "factor")));
                    let (a, b) = match factor {
                        Some(factor) => (
                            scaled(&weight, &format!("(1.0 - {factor})")),
                            scaled(&weight, &factor),
                        ),
                        None => (None, None),
                    };
                    pending.push((node.input("b").clone(), b));
                    pending.push((node.input("a").clone(), a));
                }
                NodeKind::AddShader => {
                    pending.push((node.input("b").clone(), weight.clone()));
                    pending.push((node.input("a").clone(), weight));
                }
                NodeKind::Emission if side.levels[path] == 0 => {
                    if let Some(weight) = &weight {
                        let [color, strength] =
                            ["color", "strength"].map(|port| emitter.text(node, port));
                        lines.push(format!(
                            "surface.emission += {weight} * {color} * {strength};"
                        ));
                    }
                }
                _ => {
                    let Some(Closure::Model(model)) = closure(flat, node) else {
                        continue;
                    };
                    if side.stages[path] != 0 {
                        continue;
                    }
                    let data = format!("c_{}", id_of(path));
                    let has =
                        |wanted: Field| model.fields.iter().any(|(_, field)| *field == wanted);
                    if main == Some(path) {
                        for (field, name) in [
                            (Field::Albedo, "albedo"),
                            (Field::Normal, "normal"),
                            (Field::Occlusion, "occlusion"),
                        ] {
                            if has(field) {
                                lines.push(format!("surface.{name} = {data}.{name};"));
                            }
                        }
                    }
                    let Some(weight) = &weight else {
                        continue;
                    };
                    if has(Field::Emission) {
                        lines.push(format!("surface.emission += {weight} * {data}.emission;"));
                    }
                    if model.subsurface.is_some() && scattering.insert(path) {
                        let field = format!("w_{}", id_of(path));
                        self.data.push(Member {
                            ty: "float".to_owned(),
                            name: field.clone(),
                            zero: "0.0".to_owned(),
                        });
                        lines.push(format!("surface.model.{field} = {weight};"));
                        scattered.push((model.model, id_of(path)));
                    }
                }
            }
        }
        for (path, stage) in closures {
            if *stage != 0 || scattering.contains(path) {
                continue;
            }
            if let Some(Closure::Model(ClosureCode {
                unscattered: Some(flag),
                subsurface: Some(_),
                ..
            })) = closure(flat, &flat.nodes[*path])
            {
                lines.push(format!(
                    "surface.model.c_{}.model.{flag} = true;",
                    id_of(path)
                ));
            }
        }
        if side.level(output.input("alpha")) == 0 {
            let alpha = emitter.text(output, "alpha");
            lines.push(format!("surface.alpha = {alpha};"));
        }
        let mut start = 0;
        while start < scattered.len() {
            let model = scattered[start].0;
            let run = scattered[start..]
                .iter()
                .take_while(|(found, _)| *found == model)
                .count();
            let ids = &scattered[start..start + run];
            start += run;
            if let [(_, id)] = ids {
                scatter.push(format!(
                    "rikka_scatter_add(total, rikka_closure_scattered<{model}>(pixel, surface.model.c_{id}, context), surface.model.w_{id});"
                ));
                continue;
            }
            let name = format!("scatter_{}", ids[0].1);
            scatter.push(format!(
                "RikkaSurfaceData<{model}.Data> {name}_data[{run}];"
            ));
            scatter.push(format!("float {name}_weight[{run}];"));
            for (index, (_, id)) in ids.iter().enumerate() {
                scatter.push(format!("{name}_data[{index}] = surface.model.c_{id};"));
                scatter.push(format!("{name}_weight[{index}] = surface.model.w_{id};"));
            }
            scatter.extend([
                "[loop]".to_owned(),
                format!("for (uint index = 0u; index < {run}u; index++) {{"),
                format!(
                    "    rikka_scatter_add(total, rikka_closure_scattered<{model}>(pixel, {name}_data[index], context), {name}_weight[index]);"
                ),
                "}".to_owned(),
            ]);
        }
        (lines, scatter)
    }
}

struct Text {
    text: String,
}

impl Text {
    fn line(&mut self, depth: usize, line: &str) {
        if line.is_empty() {
            self.text.push('\n');
        } else {
            writeln!(self.text, "{}{line}", "    ".repeat(depth)).expect("writing to a string");
        }
    }

    fn lines(&mut self, depth: usize, lines: &[String]) {
        for line in lines {
            self.line(depth, line);
        }
    }

    fn staged(&mut self, depth: usize, stages: &[(usize, &[String])]) {
        let live: Vec<&(usize, &[String])> = stages
            .iter()
            .filter(|(_, lines)| !lines.is_empty())
            .collect();
        match live.as_slice() {
            [] => {}
            [(_, lines)] if stages.len() == 1 => self.lines(depth, lines),
            live => {
                for (index, (stage, lines)) in live.iter().enumerate() {
                    let keyword = match index {
                        0 => "if",
                        _ => "} else if",
                    };
                    self.line(depth, &format!("{keyword} (state.stage == {stage}u) {{"));
                    self.lines(depth + 1, lines);
                }
                self.line(depth, "}");
            }
        }
    }
}

fn value_default(value: &SocketValue, ty: ValueType) -> Option<String> {
    let width = ty.width()?;
    let constant = value.number()?.resized(width);
    Some(
        constant
            .values()
            .iter()
            .map(|value| float(*value))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

pub(crate) fn texture_kind(kind: TextureKind) -> &'static str {
    match kind {
        TextureKind::Colour => "colour",
        TextureKind::Data => "data",
        TextureKind::Normal => "normal",
        TextureKind::Ramp => "ramp",
    }
}

fn generate(flat: &Flat<'_>, name: &str) -> MaterialSource {
    let graph = flat.graph;
    let output = flat.output().expect("a checked graph has an output").1;
    let mut errors = Vec::new();
    let body = stage(flat, output, false, &["surface", "alpha"], &mut errors);
    let outline = linked(output.input("outline"))
        .then(|| stage(flat, output, true, &["outline"], &mut errors));
    let mut generator = Generator {
        flat,
        data: Vec::new(),
        state: Vec::new(),
        copied: BTreeMap::new(),
        forced: BTreeSet::new(),
        featured: false,
        routed: Vec::new(),
        extras: Vec::new(),
        scatters: false,
        features: Features::default(),
    };
    let mut uniforms = Uniforms::default();
    let body_code = generator.side(&body, &mut uniforms);
    let outline_code = outline
        .as_ref()
        .map(|side| generator.side(side, &mut uniforms));
    let mut gated: BTreeMap<String, u32> = BTreeMap::new();
    for (control, bit) in generator.features.controls() {
        match control {
            Control::Socket(at) => {
                if let Some(uniform) = uniforms.list.iter_mut().find(|known| known.at == *at) {
                    uniform.attributes.push(format!("Feature({bit})"));
                }
            }
            Control::Parameter(name) => {
                gated.insert(name.clone(), bit);
            }
        }
    }
    generator.featured |= !generator.features.is_empty();

    let vertex = match output.input("displacement") {
        Source::Link(..) | Source::Socket(..) => {
            let mut emitter = Emitter::new(flat, Site::Vertex, &mut uniforms);
            let cone = cone_of(flat, output, &["displacement"]);
            emitter.run(&flat.order(&cone));
            let offset = emitter.text(output, "displacement");
            let reach = graph
                .parameters()
                .iter()
                .find(|parameter| parameter.role == Some(Role::Displacement))
                .expect("a checked displacement has a bound");
            emitter.lines.push(format!(
                "vertex.world_position += rikka_graph_bounded({offset}, this.{});",
                reach.name
            ));
            Some(emitter.lines)
        }
        _ => None,
    };
    let hull = outline.as_ref().map(|_| {
        let mut emitter = Emitter::new(flat, Site::Hull, &mut uniforms);
        let cone = cone_of(flat, output, &OUTLINE_PORTS[1..]);
        emitter.run(&flat.order(&cone));
        let [width, direction, depth] = [OUTLINE_PORTS[1], OUTLINE_PORTS[2], OUTLINE_PORTS[3]]
            .map(|port| emitter.text(output, port));
        emitter.lines.extend([
            "RikkaHull shape;".to_owned(),
            format!("shape.direction = {direction};"),
            format!("shape.width = {width} * scale;"),
            format!("shape.depth_offset = {depth};"),
            "return shape;".to_owned(),
        ]);
        emitter.lines
    });
    let coverage = match output.input("alpha") {
        Source::Link(from, _) => {
            let cone = flat.cone([from.clone()]);
            let safe = flat.site_errors(&cone, Site::Coverage).is_empty();
            safe.then(|| {
                let mut emitter = Emitter::new(flat, Site::Coverage, &mut uniforms);
                emitter.run(&flat.order(&cone));
                let alpha = emitter.text(output, "alpha");
                emitter.lines.push(format!("return {alpha};"));
                emitter.lines
            })
        }
        Source::Socket(..) => {
            let mut emitter = Emitter::new(flat, Site::Coverage, &mut uniforms);
            let alpha = emitter.text(output, "alpha");
            Some(vec![format!("return {alpha};")])
        }
        _ => None,
    };

    let mut out = Text {
        text: String::from("import rikka;\nimport rikka.graph;\nimport rikka.closure;\n"),
    };
    out.line(0, "");
    out.line(0, "struct GraphData {");
    for field in &generator.data {
        out.line(1, &format!("{} {};", field.ty, field.name));
    }
    for (name, ty) in &generator.copied {
        out.line(1, &format!("{ty} {name};"));
    }
    if generator.data.is_empty() && generator.copied.is_empty() {
        out.line(1, "uint unused;");
    }
    out.line(0, "};");
    out.line(0, "");

    let hooks = if vertex.is_some() { ", IVertex" } else { "" };
    out.line(
        0,
        &format!("struct Surface : ISurface<GraphData>{hooks} {{"),
    );
    for parameter in graph.parameters() {
        match gated.get(&parameter.name) {
            Some(bit) => out.line(1, &format!("[Feature({bit})] {}", parameter.declaration())),
            None => out.line(1, &parameter.declaration()),
        }
    }
    for uniform in &uniforms.list {
        let mut line = String::new();
        let held = graph.socket(&uniform.at);
        match uniform.ty {
            ValueType::Texture => {
                write!(line, "[Texture(\"{}\")] ", texture_kind(uniform.texture))
                    .expect("writing to a string");
            }
            ty => {
                if let Some(default) = held.and_then(|held| value_default(held, ty)) {
                    write!(line, "[Default({default})] ").expect("writing to a string");
                }
            }
        }
        line.push_str("[Hint(\"hidden\")] ");
        for attribute in &uniform.attributes {
            write!(line, "[{attribute}] ").expect("writing to a string");
        }
        write!(line, "{} {};", uniform.ty.slang(), uniform.name).expect("writing to a string");
        out.line(1, &line);
    }
    if generator.featured {
        for bit in &generator.forced {
            out.line(
                1,
                &format!(
                    "[Feature({bit})] [Hint(\"hidden\")] [Default(1.0)] float u_feature_{bit};"
                ),
            );
        }
        out.line(
            1,
            &format!(
                "[Feature({FEATURE_MARK})] [Hint(\"hidden\")] [Default(0.0)] float u_features;"
            ),
        );
    }
    out.lines(1, &generator.extras);
    out.line(0, "");
    out.line(1, "[constref]");
    out.line(
        1,
        "RikkaSurfaceData<GraphData> evaluate(RikkaPixel pixel, RikkaContext context) {",
    );
    out.line(2, "GraphData data;");
    for field in &generator.data {
        out.line(2, &format!("data.{} = {};", field.name, field.zero));
    }
    for name in generator.copied.keys() {
        out.line(2, &format!("data.{name} = this.{name};"));
    }
    if generator.data.is_empty() && generator.copied.is_empty() {
        out.line(2, "data.unused = 0u;");
    }
    out.line(
        2,
        "RikkaSurfaceData<GraphData> surface = rikka_surface_data(pixel, data);",
    );
    match &outline_code {
        Some(outline_code) => {
            out.line(2, "if (pixel.hull) {");
            out.lines(3, &outline_code.surface);
            out.line(2, "} else {");
            out.lines(3, &body_code.surface);
            out.line(2, "}");
        }
        None => out.lines(2, &body_code.surface),
    }
    out.line(2, "return surface;");
    out.line(1, "}");
    if let Some(lines) = &coverage {
        out.line(0, "");
        out.line(1, "[constref]");
        out.line(
            1,
            "override float coverage(RikkaCoverage coverage, RikkaContext context) {",
        );
        out.lines(2, lines);
        out.line(1, "}");
    }
    if let Some(lines) = &hull {
        out.line(0, "");
        out.line(1, "[constref]");
        out.line(
            1,
            "override RikkaHull hull(RikkaHullVertex vertex, RikkaContext context, float scale) {",
        );
        out.lines(2, lines);
        out.line(1, "}");
    }
    if let Some(lines) = &vertex {
        out.line(0, "");
        out.line(1, "[constref]");
        out.line(
            1,
            "void deform(inout RikkaVertex vertex, RikkaContext context, float time) {",
        );
        out.lines(2, lines);
        out.line(1, "}");
    }
    out.line(0, "};");
    out.line(0, "");

    out.line(0, "struct GraphState {");
    out.line(1, "RikkaPixel pixel;");
    out.line(1, "RikkaSurfaceData<GraphData> surface;");
    out.line(1, "uint stage;");
    for field in &generator.state {
        out.line(1, &format!("{} {};", field.ty, field.name));
    }
    out.line(0, "};");
    out.line(0, "");

    let scattering = generator.scatters && !body_code.scatter.is_empty();
    let conforms = if scattering { ", IScattering" } else { "" };
    out.line(0, &format!("struct Model : IShadingModel{conforms} {{"));
    out.line(1, "RikkaGloss gloss;");
    out.line(1, "RikkaSeeThroughTraits see_through;");
    out.line(0, "");
    out.line(1, "typealias Data = GraphData;");
    out.line(1, "typealias State = GraphState;");
    out.line(0, "");
    out.line(1, "[constref]");
    out.line(
        1,
        "State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {",
    );
    out.line(2, "State state;");
    out.line(2, "state.pixel = pixel;");
    out.line(2, "state.surface = surface;");
    out.line(2, "state.stage = 0u;");
    for field in &generator.state {
        if !field.zero.is_empty() {
            out.line(2, &format!("state.{} = {};", field.name, field.zero));
        }
    }
    out.line(2, "return state;");
    out.line(1, "}");

    let sides: Vec<(&SideCode, usize)> = std::iter::once((&body_code, 0))
        .chain(outline_code.as_ref().map(|code| (code, body.loops)))
        .collect();
    let stages = |pick: &dyn Fn(&SideCode) -> &Vec<Vec<String>>| -> Vec<(usize, Vec<String>)> {
        sides
            .iter()
            .flat_map(|(code, base)| {
                pick(code)
                    .iter()
                    .enumerate()
                    .map(|(level, lines)| (base + level, lines.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    fn borrowed(stages: &[(usize, Vec<String>)]) -> Vec<(usize, &[String])> {
        stages
            .iter()
            .map(|(stage, lines)| (*stage, lines.as_slice()))
            .collect()
    }
    for (signature, staged, overrides) in [
        (
            "void direct(inout State state, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {",
            stages(&|code| &code.direct),
            false,
        ),
        (
            "void transmit(inout State state, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {",
            stages(&|code| &code.transmit),
            true,
        ),
        (
            "void indirect(inout State state, RikkaIndirect indirect, RikkaContext context) {",
            stages(&|code| &code.indirect),
            false,
        ),
    ] {
        if overrides && staged.iter().all(|(_, lines)| lines.is_empty()) {
            continue;
        }
        out.line(0, "");
        out.line(1, "[constref]");
        match overrides {
            true => out.line(1, &format!("override {signature}")),
            false => out.line(1, signature),
        }
        out.staged(2, &borrowed(&staged));
        out.line(1, "}");
    }
    for (code, base) in &sides {
        for (index, lines) in code.combines.iter().enumerate() {
            out.line(0, "");
            out.line(1, "[constref]");
            out.line(
                1,
                &format!(
                    "void combine_{}(inout State state, RikkaContext context) {{",
                    base + index + 1
                ),
            );
            out.lines(2, lines);
            out.line(1, "}");
        }
    }
    out.line(0, "");
    out.line(1, "[constref]");
    out.line(
        1,
        "float4 resolve(State state, RikkaPixel pixel, RikkaContext context) {",
    );
    out.lines(2, &body_code.finish);
    out.line(1, "}");
    if let Some(outline_code) = &outline_code {
        out.line(0, "");
        out.line(1, "[constref]");
        out.line(
            1,
            "override float4 resolve_hull(State state, RikkaPixel pixel, RikkaContext context) {",
        );
        out.lines(2, &outline_code.finish);
        out.line(1, "}");
    }
    out.line(0, "");
    out.line(1, "[constref]");
    out.line(
        1,
        "override float4 shade(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {",
    );
    out.line(2, "State state = prepare(pixel, surface, context);");
    let run = |out: &mut Text, depth: usize, code: &SideCode, base: usize, finish: &str| {
        for (level, begin) in code.begins.iter().enumerate() {
            if level > 0 {
                out.line(depth, &format!("combine_{}(state, context);", base + level));
            }
            out.lines(depth, begin);
            if code.illuminates[level] {
                out.line(depth, &format!("state.stage = {}u;", base + level));
                out.line(depth, "rikka_illuminate(this, state, pixel, context);");
            }
        }
        out.line(depth, &format!("return {finish}(state, pixel, context);"));
    };
    if let Some(outline_code) = &outline_code {
        out.line(2, "if (pixel.hull) {");
        run(&mut out, 3, outline_code, body.loops, "resolve_hull");
        out.line(2, "}");
    }
    run(&mut out, 2, &body_code, 0, "resolve");
    out.line(1, "}");
    out.line(0, "");
    out.line(1, "[constref]");
    out.line(1, "override RikkaGBuffer gbuffer(RikkaContext context) {");
    out.line(2, "return rikka_gbuffer(context, gloss);");
    out.line(1, "}");
    out.line(0, "");
    out.line(1, "[constref]");
    out.line(
        1,
        "override RikkaSeeThrough seen_through(RikkaContext context) {",
    );
    out.line(2, "return see_through.seen();");
    out.line(1, "}");
    if scattering {
        out.line(0, "");
        out.line(1, "[constref]");
        out.line(
            1,
            "override RikkaScatter scattered(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {",
        );
        out.line(2, "RikkaScatter total = rikka_no_scatter();");
        out.lines(2, &body_code.scatter);
        out.line(2, "return total;");
        out.line(1, "}");
    }
    out.line(0, "};");

    MaterialSource {
        name: name.to_owned(),
        source: out.text,
        sockets: uniforms.list.iter().map(Socket::of).collect(),
        outlined: outline.is_some(),
        displaces: vertex.is_some(),
    }
}
