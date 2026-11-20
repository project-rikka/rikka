use std::sync::Arc;

use animate_look::{
    GraphRef, Look, LookEdit, LookKey, LookValue, Name, SlotEdit, SlotKey, SocketKey, Target,
    cinematic_composite, cinematic_world, default_composite, default_world, fresh_name, kind_name,
};
use animate_shader::empty_material;
use rikka_mmd::VmdCurve;
use rikka_shader_graph::{
    Constant, Graph, Hint, Library, NodeId, ParameterDecl, Port, PortRef, PortType, Scope,
    SocketRef, SocketValue, ValueType,
};
use rikka_slang::ParameterKind;

use crate::argument_error::ArgumentError;
use crate::look::request::{
    Assigned, Given, GraphEdits, GraphOp, KeyTarget, LookKeys, LookReset, MaterialEdits, NodeRef,
    Plug, SlotPick, SlotRef, StarterKind,
};
use crate::parse::Parsed;

const SPACING: f32 = 260.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotInfo {
    pub name: String,
    pub class: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FigureSlots {
    pub handle: String,
    pub name: Name,
    pub materials: Vec<SlotInfo>,
}

fn refused(error: impl std::fmt::Display) -> ArgumentError {
    ArgumentError::Refused(error.to_string())
}

fn at_edit(edit: usize) -> impl Fn(ArgumentError) -> ArgumentError {
    move |error| ArgumentError::AtEdit {
        edit,
        source: Box::new(error),
    }
}

fn graph_name(graph: &GraphRef) -> String {
    match graph {
        GraphRef::World => "world".to_owned(),
        GraphRef::Composite => "composite".to_owned(),
        GraphRef::Material(key) => format!("material:{key}"),
    }
}

fn wrong(what: &str, expected: impl Into<String>, found: impl Into<String>) -> ArgumentError {
    ArgumentError::WrongType {
        what: what.to_owned(),
        expected: expected.into(),
        found: found.into(),
    }
}

pub(crate) fn port_of(
    graph: &Graph,
    library: &Library,
    node: NodeId,
    port: &str,
    input: bool,
) -> Parsed<Port> {
    let held = graph
        .body()
        .node(node)
        .ok_or_else(|| ArgumentError::Unknown {
            kind: "node",
            name: node.to_string(),
            expected: "use an id the Look tool lists or a name given by an earlier add".to_owned(),
        })?;
    let ports = graph
        .ports(library, &Scope::Main, &held.kind)
        .map_err(refused)?;
    let found = match input {
        true => ports.input(port),
        false => ports.output(port),
    };
    found.cloned().ok_or_else(|| {
        let names: Vec<&str> = match input {
            true => ports.inputs.iter().map(|port| port.name.as_str()).collect(),
            false => ports
                .outputs
                .iter()
                .map(|port| port.name.as_str())
                .collect(),
        };
        ArgumentError::Unknown {
            kind: match input {
                true => "input",
                false => "output",
            },
            name: port.to_owned(),
            expected: format!(
                "{} has {}",
                held.kind.title(library),
                match names.is_empty() {
                    true => "none".to_owned(),
                    false => names.join(", "),
                }
            ),
        }
    })
}

fn width_of(port: &Port) -> Option<usize> {
    match port.ty {
        PortType::Value(ty) => ty.width(),
        PortType::Dynamic => None,
    }
}

pub(crate) fn socket_of(port: &Port, given: &Given) -> Parsed<SocketValue> {
    let takes_text = matches!(port.hint, Hint::Text | Hint::File(_) | Hint::Texture(_))
        || port.ty == PortType::Value(ValueType::Texture);
    match (given, port.hint) {
        (Given::Text(text), _) if takes_text => Ok(SocketValue::Asset(text.clone())),
        (Given::Text(option), Hint::Options(options)) => options
            .iter()
            .position(|known| *known == option.as_str())
            .map(|index| SocketValue::from(index as f32))
            .ok_or_else(|| ArgumentError::Unknown {
                kind: "option",
                name: option.clone(),
                expected: format!("{} takes {}", port.name, options.join(", ")),
            }),
        (Given::Text(text), _) => Err(wrong(&port.name, "numbers", format!("\"{text}\""))),
        (Given::Numbers(_), _) if takes_text => {
            Err(wrong(&port.name, "a file path or text", "numbers"))
        }
        (Given::Numbers(numbers), hint) => {
            if let Some(width) = width_of(port)
                && numbers.len() != 1
                && numbers.len() != width
            {
                return Err(wrong(
                    &port.name,
                    format!("1 or {width} numbers"),
                    format!("{} numbers", numbers.len()),
                ));
            }
            let numbers: Vec<f32> = match hint {
                Hint::Angle => numbers.iter().map(|degrees| degrees.to_radians()).collect(),
                _ => numbers.clone(),
            };
            if let Hint::Range(low, high) = hint
                && let Some(outside) = numbers.iter().find(|value| !(low..=high).contains(*value))
            {
                return Err(ArgumentError::OutOfRange {
                    what: port.name.clone(),
                    value: *outside,
                    min: low,
                    max: high,
                    unit: "",
                });
            }
            if let Hint::Options(options) = hint
                && numbers[0].round() as usize >= options.len()
            {
                return Err(ArgumentError::Unknown {
                    kind: "option",
                    name: numbers[0].to_string(),
                    expected: format!("{} takes {}", port.name, options.join(", ")),
                });
            }
            let constant = match (numbers.as_slice(), width_of(port)) {
                ([single], Some(width)) => Constant::from_slice(&vec![*single; width]),
                (values, _) => Constant::from_slice(values),
            };
            constant
                .map(SocketValue::Number)
                .ok_or_else(|| wrong(&port.name, "1 to 4 numbers", "more"))
        }
    }
}

fn resolve(node: &NodeRef, named: &[(String, NodeId)]) -> Parsed<NodeId> {
    match node {
        NodeRef::Id(id) => Ok(*id),
        NodeRef::Named(name) => named
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, id)| *id)
            .ok_or_else(|| ArgumentError::Unknown {
                kind: "node name",
                name: name.clone(),
                expected: "name a node in an earlier add of the same call, or use its id"
                    .to_owned(),
            }),
    }
}

fn plug_ref(plug: &Plug, named: &[(String, NodeId)]) -> Parsed<PortRef> {
    Ok(PortRef::new(resolve(&plug.node, named)?, plug.port.clone()))
}

fn free_spot(graph: &Graph) -> [f32; 2] {
    let right = graph
        .body()
        .nodes()
        .values()
        .map(|node| node.position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    match right.is_finite() {
        true => [right + SPACING, 0.0],
        false => [0.0, 0.0],
    }
}

fn set(graph: &mut Graph, library: &Library, to: PortRef, value: &Given) -> Parsed<()> {
    let port = port_of(graph, library, to.node, &to.port, true)?;
    let value = socket_of(&port, value)?;
    graph
        .edit(library, Scope::Main)
        .set_input(to.node, &to.port, Some(value))
        .map(drop)
        .map_err(refused)
}

fn apply_op(
    graph: &mut Graph,
    library: &Library,
    op: &GraphOp,
    named: &mut Vec<(String, NodeId)>,
) -> Parsed<()> {
    match op {
        GraphOp::Add {
            name,
            kind,
            at,
            values,
        } => {
            let position = at.unwrap_or_else(|| free_spot(graph));
            let id = graph
                .edit(library, Scope::Main)
                .add_node(kind.clone(), position)
                .map_err(refused)?
                .value;
            if let Some(name) = name {
                named.retain(|(held, _)| held != name);
                named.push((name.clone(), id));
            }
            for (port, value) in values {
                set(graph, library, PortRef::new(id, port.clone()), value)?;
            }
            Ok(())
        }
        GraphOp::Remove(node) => {
            let id = resolve(node, named)?;
            port_presence(graph, id)?;
            graph
                .edit(library, Scope::Main)
                .remove_node(id)
                .map(drop)
                .map_err(refused)
        }
        GraphOp::Connect { from, to } => {
            let from = plug_ref(from, named)?;
            let to = plug_ref(to, named)?;
            port_of(graph, library, from.node, &from.port, false)?;
            port_of(graph, library, to.node, &to.port, true)?;
            graph
                .edit(library, Scope::Main)
                .connect(from, to)
                .map(drop)
                .map_err(refused)
        }
        GraphOp::Disconnect { to, from } => {
            let to = plug_ref(to, named)?;
            port_of(graph, library, to.node, &to.port, true)?;
            let mut editor = graph.edit(library, Scope::Main);
            match from {
                Some(from) => editor.detach(plug_ref(from, named)?, to),
                None => editor.disconnect(to),
            }
            .map(drop)
            .map_err(refused)
        }
        GraphOp::Set { to, value } => set(graph, library, plug_ref(to, named)?, value),
        GraphOp::Unset(to) => {
            let to = plug_ref(to, named)?;
            port_of(graph, library, to.node, &to.port, true)?;
            graph
                .edit(library, Scope::Main)
                .set_input(to.node, &to.port, None)
                .map(drop)
                .map_err(refused)
        }
    }
}

fn port_presence(graph: &Graph, id: NodeId) -> Parsed<()> {
    match graph.body().node(id) {
        Some(_) => Ok(()),
        None => Err(ArgumentError::Unknown {
            kind: "node",
            name: id.to_string(),
            expected: "use an id the Look tool lists".to_owned(),
        }),
    }
}

fn checked(graph: &Graph, library: &Library, at: &GraphRef) -> Parsed<()> {
    let report = graph.validate(library);
    let problems: Vec<String> = report
        .errors()
        .iter()
        .filter(|error| !error.problem.is_incomplete())
        .map(ToString::to_string)
        .collect();
    match problems.is_empty() {
        true => Ok(()),
        false => Err(ArgumentError::Refused(format!(
            "{} would be invalid: {}",
            graph_name(at),
            problems.join("; ")
        ))),
    }
}

pub fn apply_graph_edits(
    look: &mut Look,
    library: &Library,
    edits: &GraphEdits,
) -> Parsed<Vec<(String, NodeId)>> {
    let held = look
        .graph(&edits.graph)
        .ok_or_else(|| ArgumentError::Unknown {
            kind: "graph",
            name: graph_name(&edits.graph),
            expected: format!(
                "the material graphs are {}",
                look.materials
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })?;
    let mut graph = Graph::clone(held);
    let mut named = Vec::new();
    for (at, op) in edits.ops.iter().enumerate() {
        apply_op(&mut graph, library, op, &mut named).map_err(at_edit(at))?;
    }
    checked(&graph, library, &edits.graph)?;
    look.apply(
        library,
        LookEdit::Replace {
            graph: edits.graph.clone(),
            with: Arc::new(graph),
        },
    )
    .map_err(refused)?;
    Ok(named)
}

fn figure<'a>(figures: &'a [FigureSlots], handle: &str) -> Parsed<&'a FigureSlots> {
    figures
        .iter()
        .find(|figure| figure.handle == handle)
        .ok_or_else(|| ArgumentError::Unknown {
            kind: "figure",
            name: handle.to_owned(),
            expected: format!(
                "the figures are {}",
                figures
                    .iter()
                    .map(|figure| format!("\"{}\"", figure.handle))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })
}

fn slot_index(figure: &FigureSlots, slot: &SlotRef) -> Parsed<u32> {
    let count = figure.materials.len() as u32;
    match slot {
        SlotRef::Index(index) if *index < count => Ok(*index),
        SlotRef::Index(index) => Err(ArgumentError::Unknown {
            kind: "slot",
            name: index.to_string(),
            expected: format!("\"{}\" has {count}, numbered from 0", figure.handle),
        }),
        SlotRef::Named(name) => figure
            .materials
            .iter()
            .position(|material| material.name == *name)
            .map(|index| index as u32)
            .ok_or_else(|| ArgumentError::Unknown {
                kind: "slot",
                name: name.clone(),
                expected: format!("use an index or a material name of \"{}\"", figure.handle),
            }),
    }
}

fn picked(figure: &FigureSlots, pick: &SlotPick) -> Parsed<Vec<u32>> {
    match pick {
        SlotPick::All => Ok((0..figure.materials.len() as u32).collect()),
        SlotPick::Class(class) => {
            let found: Vec<u32> = figure
                .materials
                .iter()
                .enumerate()
                .filter(|(_, material)| material.class == Some(class.as_str()))
                .map(|(index, _)| index as u32)
                .collect();
            match found.is_empty() {
                true => Err(ArgumentError::Unknown {
                    kind: "class",
                    name: class.clone(),
                    expected: format!("no slot of \"{}\" has it", figure.handle),
                }),
                false => Ok(found),
            }
        }
        SlotPick::Listed(slots) => slots.iter().map(|slot| slot_index(figure, slot)).collect(),
    }
}

fn preset_graph(library: &Library, key: &str) -> Parsed<(String, Graph)> {
    let preset = rikka_shader_presets::preset(key).ok_or_else(|| ArgumentError::Unknown {
        kind: "preset",
        name: key.to_owned(),
        expected: format!(
            "the presets are {}",
            rikka_shader_presets::presets()
                .iter()
                .map(|preset| preset.key)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    })?;
    Ok((
        preset.title.to_owned(),
        preset.graph(library).map_err(refused)?,
    ))
}

fn assign(look: &Look, figure: Name, materials: &[u32], key: Option<&str>) -> LookEdit {
    LookEdit::Slots(SlotEdit::Batch(
        materials
            .iter()
            .map(|material| {
                let mut slot = look
                    .slots
                    .slot(figure, *material)
                    .cloned()
                    .unwrap_or_default();
                slot.material = key.map(str::to_owned);
                SlotEdit::Replace {
                    figure,
                    material: *material,
                    slot: Some(slot),
                }
            })
            .collect(),
    ))
}

fn insert(look: &mut Look, library: &Library, key: String, graph: Graph) -> Parsed<String> {
    look.apply(
        library,
        LookEdit::InsertMaterial {
            key: key.clone(),
            graph: Arc::new(graph),
        },
    )
    .map_err(refused)?;
    Ok(key)
}

fn parameter<'a>(look: &'a Look, figure: &FigureSlots, key: &SlotKey) -> Parsed<&'a ParameterDecl> {
    let slot = look.slots.slot(key.figure, key.material);
    let Some(graph) = slot.and_then(|slot| slot.material.clone()) else {
        return Err(ArgumentError::Refused(format!(
            "slot {} of \"{}\" draws its imported material; assign a material graph first",
            key.material, figure.handle
        )));
    };
    look.slot_parameter(key).ok_or_else(|| {
        let names: Vec<&str> = look
            .materials
            .get(&graph)
            .map(|held| {
                held.parameters()
                    .iter()
                    .map(|decl| decl.name.as_str())
                    .collect()
            })
            .unwrap_or_default();
        ArgumentError::Unknown {
            kind: "parameter",
            name: key.parameter.to_string(),
            expected: format!(
                "material:{graph} declares {}",
                match names.is_empty() {
                    true => "none".to_owned(),
                    false => names.join(", "),
                }
            ),
        }
    })
}

fn parameter_value(decl: &ParameterDecl, given: &Given) -> Parsed<LookValue> {
    let socket = match (given, &decl.kind) {
        (Given::Text(text), ParameterKind::Texture { .. }) => SocketValue::Asset(text.clone()),
        (Given::Text(option), ParameterKind::Enum { options }) => {
            let index = options
                .iter()
                .position(|known| known == option)
                .ok_or_else(|| ArgumentError::Unknown {
                    kind: "option",
                    name: option.clone(),
                    expected: format!("{} takes {}", decl.name, options.join(", ")),
                })?;
            SocketValue::from(index as f32)
        }
        (Given::Text(text), _) => {
            return Err(wrong(&decl.name, "numbers", format!("\"{text}\"")));
        }
        (Given::Numbers(numbers), _) => {
            if let Some(range) = decl.range
                && let Some(outside) = numbers
                    .iter()
                    .find(|value| !(range.low..=range.high).contains(*value))
            {
                return Err(ArgumentError::OutOfRange {
                    what: decl.name.clone(),
                    value: *outside,
                    min: range.low,
                    max: range.high,
                    unit: "",
                });
            }
            SocketValue::Number(
                Constant::from_slice(numbers)
                    .ok_or_else(|| wrong(&decl.name, "1 to 4 numbers", "more"))?,
            )
        }
    };
    LookValue::from_parameter(&decl.kind, &socket)
        .ok_or_else(|| wrong(&decl.name, kind_name(&decl.kind), "that value"))
}

pub fn apply_materials(
    look: &mut Look,
    library: &Library,
    figures: &[FigureSlots],
    edits: &MaterialEdits,
    frame: u32,
    curve: VmdCurve,
) -> Parsed<Vec<String>> {
    let mut added = Vec::new();
    for (name, preset) in &edits.add {
        let graph = match preset {
            Some(preset) => preset_graph(library, preset)?.1,
            None => empty_material(library).map_err(refused)?,
        };
        added.push(insert(look, library, name.clone(), graph)?);
    }
    for (from, to) in &edits.rename {
        look.apply(
            library,
            LookEdit::RenameMaterial {
                from: from.clone(),
                to: to.clone(),
            },
        )
        .map_err(refused)?;
    }
    for key in &edits.remove {
        look.apply(library, LookEdit::RemoveMaterial { key: key.clone() })
            .map_err(refused)?;
    }
    for assignment in &edits.assign {
        let figure = figure(figures, &assignment.figure)?;
        let slots = picked(figure, &assignment.slots)?;
        let key = match &assignment.material {
            Assigned::Imported => None,
            Assigned::Graph(key) => {
                if !look.materials.contains_key(key) {
                    return Err(ArgumentError::Unknown {
                        kind: "material graph",
                        name: key.clone(),
                        expected: "add it first or use preset:<key>".to_owned(),
                    });
                }
                Some(key.clone())
            }
            Assigned::Preset(preset) => {
                let (title, graph) = preset_graph(library, preset)?;
                let key = fresh_name(&title, |taken| look.materials.contains_key(taken));
                added.push(insert(look, library, key.clone(), graph)?);
                Some(key)
            }
        };
        match (&key, assignment.unique) {
            (Some(shared), true) => {
                for slot in slots {
                    let graph = Graph::clone(&look.materials[shared]);
                    let copy = fresh_name(shared, |taken| look.materials.contains_key(taken));
                    added.push(insert(look, library, copy.clone(), graph)?);
                    let edit = assign(look, figure.name, &[slot], Some(&copy));
                    look.apply(library, edit).map_err(refused)?;
                }
            }
            (key, _) => {
                let edit = assign(look, figure.name, &slots, key.as_deref());
                look.apply(library, edit).map_err(refused)?;
            }
        }
    }
    for value in &edits.values {
        let figure = figure(figures, &value.figure)?;
        let key = SlotKey {
            figure: figure.name,
            material: slot_index(figure, &value.slot)?,
            parameter: Name::new(&value.parameter),
        };
        let decl = parameter(look, figure, &key)?;
        let fitted = value
            .value
            .as_ref()
            .map(|given| parameter_value(decl, given))
            .transpose()?;
        look.set_value(library, &Target::Slot(key), fitted, frame, curve)
            .map_err(refused)?;
    }
    Ok(added)
}

fn key_target(look: &Look, figures: &[FigureSlots], target: &KeyTarget) -> Parsed<Target> {
    Ok(match target {
        KeyTarget::Socket { graph, node, port } => {
            let key = SocketKey::new(
                graph.clone(),
                SocketRef::new(Scope::Main, *node, port.clone()),
            );
            if look.graph(graph).is_none() {
                return Err(ArgumentError::Unknown {
                    kind: "graph",
                    name: graph_name(graph),
                    expected: "use world, composite or a listed material".to_owned(),
                });
            }
            Target::Socket(key)
        }
        KeyTarget::Slot {
            figure: handle,
            slot,
            parameter,
        } => {
            let held = figure(figures, handle)?;
            let key = SlotKey {
                figure: held.name,
                material: slot_index(held, slot)?,
                parameter: Name::new(parameter),
            };
            self::parameter(look, held, &key)?;
            Target::Slot(key)
        }
    })
}

pub fn apply_keys(
    look: &mut Look,
    library: &Library,
    figures: &[FigureSlots],
    keys: &LookKeys,
    curve: VmdCurve,
) -> Parsed<()> {
    for removal in &keys.remove {
        let target = key_target(look, figures, &removal.target)?;
        let edit = match removal.frame {
            Some(frame) => look.keys.removing(&target, frame),
            None => look.keys.dropping(|held| *held == target),
        };
        look.apply(library, LookEdit::Keys(edit)).map_err(refused)?;
    }
    for set in &keys.set {
        let target = key_target(look, figures, &set.target)?;
        let value = match (&set.value, &target) {
            (Some(given), Target::Socket(key)) => {
                let graph = look
                    .graph(&key.graph)
                    .ok_or_else(|| refused("the graph vanished"))?;
                let port = port_of(graph, library, key.socket.node, &key.socket.port, true)?;
                let socket = socket_of(&port, given)?;
                LookValue::from_socket(&socket, port.hint)
            }
            (Some(given), Target::Slot(key)) => {
                let held = figures
                    .iter()
                    .find(|figure| figure.name == key.figure)
                    .ok_or_else(|| refused("the figure vanished"))?;
                parameter_value(parameter(look, held, key)?, given)?
            }
            (None, target) => look
                .value_at(library, target, set.frame as f32)
                .or_else(|| look.default_value(library, target))
                .ok_or_else(|| refused(format!("{target} has no value to key; give one")))?,
        };
        let curve = set.curve.unwrap_or(curve);
        match &target {
            Target::Socket(key) => {
                let socket = value
                    .to_socket()
                    .ok_or_else(|| refused(format!("{target} cannot hold a key")))?;
                look.key_socket(library, key.clone(), set.frame, socket, curve)
                    .map_err(refused)?;
            }
            Target::Slot(_) => {
                let edit = look.keys.inserting(LookKey {
                    target: target.clone(),
                    frame: set.frame,
                    value,
                    curve,
                });
                look.apply(library, LookEdit::Keys(edit)).map_err(refused)?;
            }
        }
    }
    Ok(())
}

pub fn reset_look(look: &mut Look, library: &Library, reset: LookReset) -> Parsed<()> {
    let starters = [
        (GraphRef::World, reset.world),
        (GraphRef::Composite, reset.composite),
    ];
    for (graph, starter) in starters {
        let Some(starter) = starter else {
            continue;
        };
        let built = match (&graph, starter) {
            (GraphRef::World, StarterKind::Bare) => default_world(library),
            (GraphRef::World, StarterKind::Cinematic) => cinematic_world(library),
            (_, StarterKind::Bare) => default_composite(library),
            (_, StarterKind::Cinematic) => cinematic_composite(library),
        }
        .map_err(refused)?;
        look.apply(
            library,
            LookEdit::Replace {
                graph,
                with: Arc::new(built),
            },
        )
        .map_err(refused)?;
    }
    Ok(())
}
