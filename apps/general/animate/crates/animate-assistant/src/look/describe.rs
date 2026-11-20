use animate_look::{GraphRef, Look, LookValue, Target};
use rikka_shader_graph::{Graph, GraphKind, Hint, Library, Port, PortDefault, Scope, SocketValue};
use serde_json::{Map, Value as Json, json};

use crate::argument_error::ArgumentError;
use crate::look::apply::FigureSlots;
use crate::look::request::NodesQuery;
use crate::parse::Parsed;
use crate::vocabulary::number;

fn round(value: f32) -> Json {
    number(value).parse().map_or(Json::Null, Json::Number)
}

fn graph_name(graph: &GraphRef) -> String {
    match graph {
        GraphRef::World => "world".to_owned(),
        GraphRef::Composite => "composite".to_owned(),
        GraphRef::Material(key) => format!("material:{key}"),
    }
}

fn shown(value: &SocketValue, hint: Hint) -> Json {
    match value {
        SocketValue::Asset(text) => Json::String(text.clone()),
        SocketValue::Number(constant) => {
            let numbers: Vec<Json> = constant
                .values()
                .iter()
                .map(|value| match hint {
                    Hint::Angle => round(value.to_degrees()),
                    _ => round(*value),
                })
                .collect();
            match (hint, numbers.as_slice()) {
                (Hint::Options(options), [index]) => index
                    .as_f64()
                    .and_then(|index| options.get(index.round() as usize))
                    .map_or(index.clone(), |option| Json::String((*option).to_owned())),
                (Hint::Toggle, [on]) => Json::Bool(on.as_f64().is_some_and(|on| on > 0.5)),
                (_, [single]) => single.clone(),
                _ => Json::Array(numbers),
            }
        }
    }
}

fn look_value(value: &LookValue) -> Json {
    match value {
        LookValue::Float(value) => round(*value),
        LookValue::Vec2(values) => values.iter().copied().map(round).collect(),
        LookValue::Vec3(values) => values.iter().copied().map(round).collect(),
        LookValue::Vec4(values) => values.iter().copied().map(round).collect(),
        LookValue::Int(value) => json!(value),
        LookValue::UInt(value) | LookValue::Choice(value) => json!(value),
        LookValue::Bool(on) => json!(on),
        LookValue::Enum(option) => json!(option.as_str()),
        LookValue::Asset(path) => json!(path.as_str()),
        LookValue::Texture(texture) => texture
            .as_ref()
            .map_or(Json::Null, |texture| json!(texture.asset.as_str())),
    }
}

fn port_doc(port: &Port) -> Json {
    let mut entry = Map::new();
    entry.insert("name".into(), json!(port.name));
    if let Ok(ty) = serde_json::to_value(port.ty) {
        entry.insert("type".into(), ty);
    }
    match &port.default {
        PortDefault::Value(constant) => {
            entry.insert(
                "default".into(),
                shown(&SocketValue::Number(*constant), port.hint),
            );
        }
        PortDefault::Input(input) => {
            entry.insert("default".into(), json!(input.title()));
        }
        PortDefault::Required => {
            entry.insert("required".into(), json!(true));
        }
        PortDefault::Empty => {}
    }
    match port.hint {
        Hint::Angle => {
            entry.insert("unit".into(), json!("degrees"));
        }
        Hint::Range(low, high) => {
            entry.insert("range".into(), json!([round(low), round(high)]));
        }
        Hint::Soft(low, high) => {
            entry.insert("usual".into(), json!([round(low), round(high)]));
        }
        Hint::Options(options) => {
            entry.insert("options".into(), json!(options));
        }
        Hint::Toggle => {
            entry.insert("toggle".into(), json!(true));
        }
        Hint::Text => {
            entry.insert("text".into(), json!(true));
        }
        Hint::File(extensions) => {
            entry.insert("files".into(), json!(extensions));
        }
        Hint::Texture(_) => {
            entry.insert("files".into(), json!("image"));
        }
        Hint::None | Hint::Color | Hint::Integer => {}
    }
    if port.many {
        entry.insert("many".into(), json!(true));
    }
    Json::Object(entry)
}

pub fn overview(
    look: &Look,
    figures: &[FigureSlots],
    status: &dyn Fn(&str) -> Option<String>,
) -> Json {
    let graphs: Vec<Json> = look
        .graphs()
        .map(|(at, graph)| {
            let mut entry = json!({
                "graph": graph_name(&at),
                "nodes": graph.body().nodes().len(),
            });
            if let GraphRef::Material(key) = &at {
                entry["users"] = json!(
                    look.slots
                        .iter()
                        .filter(|(_, _, slot)| slot.material.as_deref() == Some(key.as_str()))
                        .count()
                );
                if let Some(status) = status(key) {
                    entry["status"] = json!(status);
                }
            }
            entry
        })
        .collect();
    let keyed: Vec<Json> = look
        .keys
        .tracks()
        .tracks()
        .map(|(target, keys)| {
            json!({
                "target": target_json(target, figures),
                "keys": keys.iter().map(|key| key.frame).collect::<Vec<_>>(),
            })
        })
        .collect();
    let presets: Vec<Json> = rikka_shader_presets::presets()
        .iter()
        .map(|preset| {
            json!({
                "key": preset.key,
                "title": preset.title,
                "family": preset.family.name(),
                "summary": preset.summary,
            })
        })
        .collect();
    json!({
        "graphs": graphs,
        "figures": figures.iter().map(|figure| figure_slots(look, figure)).collect::<Vec<_>>(),
        "keyed": keyed,
        "presets": presets,
    })
}

fn target_json(target: &Target, figures: &[FigureSlots]) -> Json {
    match target {
        Target::Socket(key) => json!({
            "graph": graph_name(&key.graph),
            "node": key.socket.node.get(),
            "port": key.socket.port,
        }),
        Target::Slot(key) => json!({
            "figure": figures
                .iter()
                .find(|figure| figure.name == key.figure)
                .map_or_else(|| key.figure.to_string(), |figure| figure.handle.clone()),
            "slot": key.material,
            "parameter": key.parameter.as_str(),
        }),
    }
}

fn figure_slots(look: &Look, figure: &FigureSlots) -> Json {
    let slots: Vec<Json> = figure
        .materials
        .iter()
        .enumerate()
        .map(|(index, material)| {
            let slot = look.slots.slot(figure.name, index as u32);
            let mut entry = json!({
                "slot": index,
                "name": material.name,
                "material": slot
                    .and_then(|slot| slot.material.as_deref())
                    .map_or(Json::Null, |key| json!(key)),
            });
            if let Some(class) = material.class {
                entry["class"] = json!(class);
            }
            if let Some(slot) = slot
                && !slot.values.is_empty()
            {
                entry["values"] = slot
                    .values
                    .iter()
                    .map(|(name, value)| (name.to_string(), look_value(value)))
                    .collect::<Map<String, Json>>()
                    .into();
            }
            entry
        })
        .collect();
    json!({ "figure": figure.handle, "slots": slots })
}

pub fn describe_figure(look: &Look, figures: &[FigureSlots], handle: &str) -> Parsed<Json> {
    let figure = figures
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
        })?;
    let mut described = figure_slots(look, figure);
    let used: std::collections::BTreeSet<&str> = look
        .slots
        .figure(figure.name)
        .filter_map(|(_, slot)| slot.material.as_deref())
        .collect();
    described["parameters"] = used
        .into_iter()
        .filter_map(|key| Some((key, look.materials.get(key)?)))
        .map(|(key, graph)| (key.to_owned(), parameters(graph)))
        .collect::<Map<String, Json>>()
        .into();
    Ok(described)
}

fn parameters(graph: &Graph) -> Json {
    graph
        .parameters()
        .iter()
        .map(|decl| {
            let mut entry =
                json!({ "name": decl.name, "kind": animate_look::kind_name(&decl.kind) });
            if let Some(default) = decl.default
                && let Ok(default) = serde_json::to_value(default)
            {
                entry["default"] = default;
            }
            if let Some(range) = decl.range {
                entry["range"] = json!([round(range.low), round(range.high)]);
            }
            if let rikka_slang::ParameterKind::Enum { options } = &decl.kind {
                entry["options"] = json!(options);
            }
            entry
        })
        .collect()
}

pub fn describe_graph(
    look: &Look,
    library: &Library,
    at: &GraphRef,
    problems: Vec<String>,
) -> Parsed<Json> {
    let graph = look.graph(at).ok_or_else(|| ArgumentError::Unknown {
        kind: "graph",
        name: graph_name(at),
        expected: format!(
            "the material graphs are {}",
            look.materials
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ),
    })?;
    let body = graph.body();
    let nodes: Vec<Json> = body
        .nodes()
        .iter()
        .map(|(id, node)| {
            let ports = graph.ports(library, &Scope::Main, &node.kind).ok();
            let inputs: Map<String, Json> = ports
                .iter()
                .flat_map(|ports| ports.inputs.iter())
                .map(|port| {
                    let to = rikka_shader_graph::PortRef::new(*id, port.name.clone());
                    let sources: Vec<Json> = body
                        .sources(&to)
                        .map(|from| json!([from.node.get(), from.port]))
                        .collect();
                    let value = match (sources.is_empty(), node.inputs.get(&port.name)) {
                        (false, _) => json!({ "from": sources }),
                        (true, Some(value)) => json!({ "value": shown(value, port.hint) }),
                        (true, None) => match port.default {
                            PortDefault::Value(constant) => json!({
                                "default": shown(&SocketValue::Number(constant), port.hint)
                            }),
                            PortDefault::Input(input) => json!({ "default": input.title() }),
                            PortDefault::Required => json!({ "required": true }),
                            PortDefault::Empty => json!({}),
                        },
                    };
                    (port.name.clone(), value)
                })
                .collect();
            let outputs: Vec<&str> = ports
                .iter()
                .flat_map(|ports| ports.outputs.iter())
                .map(|port| port.name.as_str())
                .collect();
            json!({
                "id": id.get(),
                "kind": serde_json::to_value(&node.kind).unwrap_or(Json::Null),
                "title": node.kind.title(library),
                "inputs": inputs,
                "outputs": outputs,
            })
        })
        .collect();
    let mut described = json!({
        "graph": graph_name(at),
        "kind": graph.kind().name(),
        "nodes": nodes,
    });
    if graph.kind() == GraphKind::Material {
        described["parameters"] = parameters(graph);
    }
    if !graph.functions().is_empty() {
        described["groups"] = json!(graph.functions().keys().collect::<Vec<_>>());
    }
    if !problems.is_empty() {
        described["problems"] = json!(problems);
    }
    Ok(described)
}

pub fn catalog(look: &Look, library: &Library, query: &NodesQuery) -> Json {
    let blank;
    let graph = match query.kind {
        GraphKind::World => &*look.world,
        GraphKind::Composite => &*look.composite,
        GraphKind::Material => {
            blank = Graph::new(GraphKind::Material);
            &blank
        }
    };
    let search = query.search.as_deref().map(str::to_lowercase);
    let entries: Vec<Json> = graph
        .catalog(library, &Scope::Main)
        .into_iter()
        .filter(|entry| entry.kind != rikka_shader_graph::NodeKind::Output)
        .filter_map(|entry| {
            let kind = serde_json::to_value(&entry.kind).ok()?;
            let Some(search) = &search else {
                return Some(json!({ "kind": kind, "title": entry.title }));
            };
            let haystack = format!("{} {} {}", kind, entry.title, entry.doc).to_lowercase();
            haystack.contains(search.as_str()).then(|| {
                json!({
                    "kind": kind,
                    "title": entry.title,
                    "doc": entry.doc,
                    "inputs": entry.ports.inputs.iter().map(port_doc).collect::<Vec<_>>(),
                    "outputs": entry.ports.outputs.iter().map(|port| port.name.clone()).collect::<Vec<_>>(),
                })
            })
        })
        .collect();
    json!({ "nodes": entries })
}
