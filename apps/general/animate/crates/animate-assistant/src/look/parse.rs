use std::collections::BTreeMap;

use animate_look::{GraphRef, Name, SlotKey, SocketKey, Target};
use rikka_shader_graph::{GraphKind, NodeId, NodeKind, Scope, SocketRef};
use serde::Deserialize;
use serde_json::Value as Json;

use crate::argument_error::ArgumentError;
use crate::look::request::{
    Assigned, Assignment, Given, GraphEdits, GraphOp, KeyRemoval, KeySet, KeyTarget, LookKeys,
    LookQuery, LookReset, MaterialEdits, NodeRef, NodesQuery, Plug, SlotPick, SlotRef, SlotValue,
    StarterKind,
};
use crate::parse::{Parsed, curve, read, unknown};
use crate::request::ToolRequest;

const MATERIAL: &str = "material:";
const PRESET: &str = "preset:";
const CLASS: &str = "class:";
const ALL: &str = "all";

fn wrong(what: &str, expected: &str, found: &Json) -> ArgumentError {
    ArgumentError::WrongType {
        what: what.to_owned(),
        expected: expected.to_owned(),
        found: found.to_string(),
    }
}

pub(crate) fn graph_ref(name: &str) -> Parsed<GraphRef> {
    match name {
        "world" => Ok(GraphRef::World),
        "composite" => Ok(GraphRef::Composite),
        other => match other.strip_prefix(MATERIAL) {
            Some(key) if !key.is_empty() => Ok(GraphRef::Material(key.to_owned())),
            _ => Err(unknown(
                "graph",
                other,
                "use world, composite or material:<name>".to_owned(),
            )),
        },
    }
}

pub(crate) fn node_id(id: u64, what: &str) -> Parsed<NodeId> {
    serde_json::from_value(Json::from(id)).map_err(|_| wrong(what, "a node id", &Json::from(id)))
}

fn node_ref(input: &Json, what: &str) -> Parsed<NodeRef> {
    match input {
        Json::Number(number) => number
            .as_u64()
            .ok_or_else(|| wrong(what, "a node id or name", input))
            .and_then(|id| node_id(id, what))
            .map(NodeRef::Id),
        Json::String(name) => Ok(NodeRef::Named(name.clone())),
        other => Err(wrong(
            what,
            "a node id or a name given by an earlier add",
            other,
        )),
    }
}

fn plug(input: &Json, what: &str) -> Parsed<Plug> {
    let Json::Array(pair) = input else {
        return Err(wrong(what, "[node, port]", input));
    };
    let [node, Json::String(port)] = pair.as_slice() else {
        return Err(wrong(what, "[node, port]", input));
    };
    Ok(Plug {
        node: node_ref(node, what)?,
        port: port.clone(),
    })
}

pub(crate) fn given(input: &Json, what: &str) -> Parsed<Given> {
    let number = |value: &Json| {
        value
            .as_f64()
            .map(|value| value as f32)
            .filter(|value| value.is_finite())
            .ok_or_else(|| wrong(what, "finite numbers", input))
    };
    match input {
        Json::Bool(on) => Ok(Given::Numbers(vec![if *on { 1.0 } else { 0.0 }])),
        Json::Number(_) => Ok(Given::Numbers(vec![number(input)?])),
        Json::Array(values) if (1..=4).contains(&values.len()) => Ok(Given::Numbers(
            values.iter().map(number).collect::<Parsed<_>>()?,
        )),
        Json::String(text) => Ok(Given::Text(text.clone())),
        other => Err(wrong(
            what,
            "a number, 2 to 4 numbers, true or false, or text",
            other,
        )),
    }
}

fn kind(input: &Json) -> Parsed<NodeKind> {
    match input {
        Json::String(id) if id.contains('.') => Ok(NodeKind::Extern(id.clone())),
        other => serde_json::from_value(other.clone())
            .map_err(|_| wrong("add", "a node kind as the Nodes tool lists it", other)),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookArgs {
    #[serde(default)]
    graph: Option<String>,
    #[serde(default)]
    figure: Option<String>,
}

pub(crate) fn look(input: Json) -> Parsed<ToolRequest> {
    let args: LookArgs = read(input)?;
    Ok(ToolRequest::Look(match (args.graph, args.figure) {
        (Some(graph), _) => LookQuery::Graph(graph_ref(&graph)?),
        (None, Some(figure)) => LookQuery::Figure(figure),
        (None, None) => LookQuery::Overview,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodesArgs {
    graph: String,
    #[serde(default)]
    search: Option<String>,
}

pub(crate) fn nodes(input: Json) -> Parsed<ToolRequest> {
    let args: NodesArgs = read(input)?;
    let kind = match args.graph.as_str() {
        "world" => GraphKind::World,
        "composite" => GraphKind::Composite,
        "material" => GraphKind::Material,
        other => {
            return Err(unknown(
                "graph kind",
                other,
                "use world, composite or material".to_owned(),
            ));
        }
    };
    Ok(ToolRequest::Nodes(NodesQuery {
        kind,
        search: args.search.filter(|search| !search.trim().is_empty()),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditArgs {
    graph: String,
    edits: Vec<BTreeMap<String, Json>>,
}

fn field<'a>(
    op: &'a BTreeMap<String, Json>,
    name: &'static str,
    action: &'static str,
) -> Parsed<&'a Json> {
    op.get(name).ok_or(ArgumentError::MissingField {
        action,
        field: name,
    })
}

fn graph_op(op: &BTreeMap<String, Json>) -> Parsed<GraphOp> {
    let allowed: &[&str] = if op.contains_key("add") {
        &["add", "name", "at", "values"]
    } else if op.contains_key("connect") || op.contains_key("set") {
        &["connect", "set", "to", "value"]
    } else {
        &["remove", "disconnect", "from", "unset"]
    };
    if let Some(stray) = op.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(unknown(
            "edit field",
            stray,
            "an edit is one of add, remove, connect, disconnect, set or unset".to_owned(),
        ));
    }
    if let Some(added) = op.get("add") {
        let name = match op.get("name") {
            Some(Json::String(name)) => Some(name.clone()),
            Some(other) => return Err(wrong("name", "text", other)),
            None => None,
        };
        let at = match op.get("at") {
            Some(Json::Array(pair)) => match pair.as_slice() {
                [x, y] => Some([
                    x.as_f64()
                        .ok_or_else(|| wrong("at", "[x, y]", &Json::Array(pair.clone())))?
                        as f32,
                    y.as_f64()
                        .ok_or_else(|| wrong("at", "[x, y]", &Json::Array(pair.clone())))?
                        as f32,
                ]),
                _ => return Err(wrong("at", "[x, y]", &Json::Array(pair.clone()))),
            },
            Some(other) => return Err(wrong("at", "[x, y]", other)),
            None => None,
        };
        let values = match op.get("values") {
            Some(Json::Object(values)) => values
                .iter()
                .map(|(port, value)| Ok((port.clone(), given(value, port)?)))
                .collect::<Parsed<_>>()?,
            Some(other) => return Err(wrong("values", "an object of port values", other)),
            None => Vec::new(),
        };
        return Ok(GraphOp::Add {
            name,
            kind: kind(added)?,
            at,
            values,
        });
    }
    if let Some(node) = op.get("remove") {
        return Ok(GraphOp::Remove(node_ref(node, "remove")?));
    }
    if let Some(from) = op.get("connect") {
        return Ok(GraphOp::Connect {
            from: plug(from, "connect")?,
            to: plug(field(op, "to", "connect")?, "to")?,
        });
    }
    if let Some(to) = op.get("disconnect") {
        return Ok(GraphOp::Disconnect {
            to: plug(to, "disconnect")?,
            from: op.get("from").map(|from| plug(from, "from")).transpose()?,
        });
    }
    if let Some(to) = op.get("set") {
        return Ok(GraphOp::Set {
            to: plug(to, "set")?,
            value: given(field(op, "value", "set")?, "value")?,
        });
    }
    if let Some(to) = op.get("unset") {
        return Ok(GraphOp::Unset(plug(to, "unset")?));
    }
    Err(ArgumentError::Empty {
        expected: "one of add, remove, connect, disconnect, set or unset in every edit",
    })
}

pub(crate) fn edit_graph(input: Json) -> Parsed<ToolRequest> {
    let args: EditArgs = read(input)?;
    if args.edits.is_empty() {
        return Err(ArgumentError::Empty {
            expected: "at least one edit",
        });
    }
    let ops = args
        .edits
        .iter()
        .enumerate()
        .map(|(at, op)| {
            graph_op(op).map_err(|source| ArgumentError::AtEdit {
                edit: at,
                source: Box::new(source),
            })
        })
        .collect::<Parsed<_>>()?;
    Ok(ToolRequest::EditGraph(GraphEdits {
        graph: graph_ref(&args.graph)?,
        ops,
    }))
}

fn slot_ref(input: &Json) -> Parsed<SlotRef> {
    match input {
        Json::Number(number) => number
            .as_u64()
            .and_then(|index| u32::try_from(index).ok())
            .map(SlotRef::Index)
            .ok_or_else(|| wrong("slot", "a material index or name", input)),
        Json::String(name) => Ok(SlotRef::Named(name.clone())),
        other => Err(wrong("slot", "a material index or name", other)),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddArgs {
    name: String,
    #[serde(default)]
    preset: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameArgs {
    from: String,
    to: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssignArgs {
    figure: String,
    slots: Json,
    material: Option<String>,
    #[serde(default)]
    unique: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueArgs {
    figure: String,
    slot: Json,
    parameter: String,
    value: Option<Json>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialsArgs {
    #[serde(default)]
    add: Vec<AddArgs>,
    #[serde(default)]
    remove: Vec<String>,
    #[serde(default)]
    rename: Vec<RenameArgs>,
    #[serde(default)]
    assign: Vec<AssignArgs>,
    #[serde(default)]
    values: Vec<ValueArgs>,
}

fn slot_pick(input: &Json) -> Parsed<SlotPick> {
    match input {
        Json::String(all) if all == ALL => Ok(SlotPick::All),
        Json::String(pick) => match pick.strip_prefix(CLASS) {
            Some(class) => Ok(SlotPick::Class(class.to_owned())),
            None => Ok(SlotPick::Listed(vec![SlotRef::Named(pick.clone())])),
        },
        Json::Array(slots) => Ok(SlotPick::Listed(
            slots.iter().map(slot_ref).collect::<Parsed<_>>()?,
        )),
        other => Ok(SlotPick::Listed(vec![slot_ref(other)?])),
    }
}

pub(crate) fn materials(input: Json) -> Parsed<ToolRequest> {
    let args: MaterialsArgs = read(input)?;
    let edits = MaterialEdits {
        add: args
            .add
            .into_iter()
            .map(|add| (add.name, add.preset))
            .collect(),
        remove: args.remove,
        rename: args
            .rename
            .into_iter()
            .map(|rename| (rename.from, rename.to))
            .collect(),
        assign: args
            .assign
            .into_iter()
            .map(|assign| {
                Ok(Assignment {
                    slots: slot_pick(&assign.slots)?,
                    material: match assign.material {
                        None => Assigned::Imported,
                        Some(name) => match name.strip_prefix(PRESET) {
                            Some(preset) => Assigned::Preset(preset.to_owned()),
                            None => Assigned::Graph(name),
                        },
                    },
                    figure: assign.figure,
                    unique: assign.unique,
                })
            })
            .collect::<Parsed<_>>()?,
        values: args
            .values
            .into_iter()
            .map(|value| {
                Ok(SlotValue {
                    slot: slot_ref(&value.slot)?,
                    value: value
                        .value
                        .as_ref()
                        .map(|held| given(held, &value.parameter))
                        .transpose()?,
                    figure: value.figure,
                    parameter: value.parameter,
                })
            })
            .collect::<Parsed<_>>()?,
    };
    if edits == MaterialEdits::default() {
        return Err(ArgumentError::Empty {
            expected: "add, remove, rename, assign or values",
        });
    }
    Ok(ToolRequest::Materials(edits))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetArgs {
    #[serde(default)]
    graph: Option<String>,
    #[serde(default)]
    node: Option<u64>,
    #[serde(default)]
    port: Option<String>,
    #[serde(default)]
    figure: Option<String>,
    #[serde(default)]
    slot: Option<Json>,
    #[serde(default)]
    parameter: Option<String>,
}

fn key_target(input: &Json) -> Parsed<KeyTarget> {
    let args: TargetArgs = read(input.clone())?;
    match args {
        TargetArgs {
            graph: Some(graph),
            node: Some(node),
            port: Some(port),
            figure: None,
            slot: None,
            parameter: None,
        } => Ok(KeyTarget::Socket {
            graph: graph_ref(&graph)?,
            node: node_id(node, "node")?,
            port,
        }),
        TargetArgs {
            graph: None,
            node: None,
            port: None,
            figure: Some(figure),
            slot: Some(slot),
            parameter: Some(parameter),
        } => Ok(KeyTarget::Slot {
            figure,
            slot: slot_ref(&slot)?,
            parameter,
        }),
        _ => Err(wrong(
            "target",
            "{graph, node, port} or {figure, slot, parameter}",
            input,
        )),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeySetArgs {
    target: Json,
    frame: u32,
    #[serde(default)]
    value: Option<Json>,
    #[serde(default)]
    curve: Option<Json>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyRemoveArgs {
    target: Json,
    #[serde(default)]
    frame: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookKeysArgs {
    #[serde(default)]
    set: Vec<KeySetArgs>,
    #[serde(default)]
    remove: Vec<KeyRemoveArgs>,
}

pub(crate) fn look_keys(input: Json) -> Parsed<ToolRequest> {
    let args: LookKeysArgs = read(input)?;
    if args.set.is_empty() && args.remove.is_empty() {
        return Err(ArgumentError::Empty {
            expected: "keys to `set` or `remove`",
        });
    }
    Ok(ToolRequest::LookKeys(LookKeys {
        set: args
            .set
            .iter()
            .map(|key| {
                Ok(KeySet {
                    target: key_target(&key.target)?,
                    frame: key.frame,
                    value: key
                        .value
                        .as_ref()
                        .map(|value| given(value, "value"))
                        .transpose()?,
                    curve: key.curve.as_ref().map(curve).transpose()?,
                })
            })
            .collect::<Parsed<_>>()?,
        remove: args
            .remove
            .iter()
            .map(|key| {
                Ok(KeyRemoval {
                    target: key_target(&key.target)?,
                    frame: key.frame,
                })
            })
            .collect::<Parsed<_>>()?,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetArgs {
    #[serde(default)]
    world: Option<String>,
    #[serde(default)]
    composite: Option<String>,
}

fn starter(name: &str) -> Parsed<StarterKind> {
    match name {
        "bare" => Ok(StarterKind::Bare),
        "cinematic" => Ok(StarterKind::Cinematic),
        other => Err(unknown(
            "starter",
            other,
            "use bare or cinematic".to_owned(),
        )),
    }
}

pub(crate) fn look_reset(input: Json) -> Parsed<ToolRequest> {
    let args: ResetArgs = read(input)?;
    let reset = LookReset {
        world: args.world.as_deref().map(starter).transpose()?,
        composite: args.composite.as_deref().map(starter).transpose()?,
    };
    if reset == LookReset::default() {
        return Err(ArgumentError::Empty {
            expected: "a `world` or `composite` starter",
        });
    }
    Ok(ToolRequest::ResetLook(reset))
}

pub(crate) fn look_lane(input: &Json) -> Parsed<Target> {
    match key_target(input)? {
        KeyTarget::Socket { graph, node, port } => Ok(Target::Socket(SocketKey::new(
            graph,
            SocketRef::new(Scope::Main, node, port),
        ))),
        KeyTarget::Slot {
            figure,
            slot: SlotRef::Index(material),
            parameter,
        } => Ok(Target::Slot(SlotKey {
            figure: Name::new(&figure),
            material,
            parameter: Name::new(&parameter),
        })),
        KeyTarget::Slot { .. } => Err(wrong("lane", "a slot by index", input)),
    }
}
