use animate_assistant::{
    FigureSlots, GraphEdits, LookKeys, LookQuery, LookReset, MaterialEdits, NodesQuery, SlotInfo,
    apply_graph_edits, apply_keys, apply_materials, catalog, describe_figure, describe_graph,
    overview, reset_look,
};
use animate_edit::KeyKind;
use animate_look::{GraphRef, Name, Target};
use animate_shader::Status;
use rikka_model::mmd::VmdCurve;
use rikka_tool::Reply;
use serde_json::{Value as Json, json};

use super::ToolResult;
use crate::animate::Animate;

fn graph_name(graph: &GraphRef) -> String {
    match graph {
        GraphRef::World => "world".to_owned(),
        GraphRef::Composite => "composite".to_owned(),
        GraphRef::Material(key) => format!("material:{key}"),
    }
}

fn status_text(status: &Status) -> String {
    match status {
        Status::Compiling => "compiling".to_owned(),
        Status::Ready => "ready".to_owned(),
        Status::Failed(diagnostics) => format!(
            "failed: {}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        ),
    }
}

impl Animate {
    fn figure_slots(&self) -> Vec<FigureSlots> {
        self.handles()
            .into_iter()
            .filter_map(|(handle, id)| {
                let figure = self.cast.figure(id)?;
                Some(FigureSlots {
                    handle,
                    name: Name::new(&figure.name),
                    materials: figure
                        .model
                        .materials
                        .iter()
                        .enumerate()
                        .map(|(index, material)| SlotInfo {
                            name: material.name.clone(),
                            class: figure.material_class(index as u32),
                        })
                        .collect(),
                })
            })
            .collect()
    }

    fn graph_problems(&self, graph: &GraphRef) -> Vec<String> {
        let look = self.look();
        let planned = match graph {
            GraphRef::World => self.workbench.world_plan(look).map(drop),
            GraphRef::Composite => self.workbench.composite_plan(look).map(drop),
            GraphRef::Material(key) => match self.shaders.lock().status(key) {
                Some(Status::Failed(diagnostics)) => Err(diagnostics),
                _ => Ok(()),
            },
        };
        match planned {
            Ok(()) => Vec::new(),
            Err(diagnostics) => diagnostics.iter().map(ToString::to_string).collect(),
        }
    }

    pub(super) fn read_look(&self, query: &LookQuery) -> ToolResult {
        let look = self.look();
        let figures = self.figure_slots();
        let described = match query {
            LookQuery::Overview => {
                let shaders = self.shaders.lock();
                overview(look, &figures, &|key| {
                    shaders.status(key).map(|status| status_text(&status))
                })
            }
            LookQuery::Graph(graph) => describe_graph(
                look,
                self.workbench.library(),
                graph,
                self.graph_problems(graph),
            )?,
            LookQuery::Figure(figure) => describe_figure(look, &figures, figure)?,
        };
        Ok(Reply::json(&described))
    }

    pub(super) fn read_nodes(&self, query: &NodesQuery) -> ToolResult {
        Ok(Reply::json(&catalog(
            self.look(),
            self.workbench.library(),
            query,
        )))
    }

    fn look_curve(&self) -> VmdCurve {
        self.curves.value(KeyKind::Look)
    }

    pub(super) fn edit_graph(&mut self, edits: &GraphEdits) -> ToolResult {
        let named = self.change_look(None, |look, library| {
            apply_graph_edits(look, library, edits)
        })?;
        let added: serde_json::Map<String, Json> = named
            .into_iter()
            .map(|(name, id)| (name, json!(id.get())))
            .collect();
        Ok(Reply::json(&json!({
            "graph": graph_name(&edits.graph),
            "added": added,
            "problems": self.graph_problems(&edits.graph),
        })))
    }

    pub(super) fn set_materials(&mut self, edits: &MaterialEdits) -> ToolResult {
        let figures = self.figure_slots();
        let frame = self.transport.frame();
        let curve = self.look_curve();
        let added = self.change_look(None, |look, library| {
            apply_materials(look, library, &figures, edits, frame, curve)
        })?;
        Ok(Reply::json(&json!({ "created": added })))
    }

    pub(super) fn key_look(&mut self, keys: &LookKeys) -> ToolResult {
        let figures = self.figure_slots();
        let curve = self.look_curve();
        self.change_look(None, |look, library| {
            apply_keys(look, library, &figures, keys, curve)
        })?;
        Ok(Reply::new("keyed"))
    }

    pub(super) fn reset_look_graphs(&mut self, reset: LookReset) -> ToolResult {
        self.change_look(None, |look, library| reset_look(look, library, reset))?;
        Ok(Reply::new("reset"))
    }

    pub(super) fn look_key_value(&self, key: &animate_look::LookKey) -> Json {
        self.look()
            .value_to_socket(&key.target, &key.value)
            .map_or(Json::Null, |value| match value {
                rikka_shader_graph::SocketValue::Number(constant) => match constant.values() {
                    [single] => super::round(*single),
                    values => values.iter().copied().map(super::round).collect(),
                },
                rikka_shader_graph::SocketValue::Asset(path) => json!(path),
            })
    }

    pub(super) fn look_summary(&self) -> Json {
        let look = self.look();
        json!({
            "graphs": look.graphs().map(|(at, _)| graph_name(&at)).collect::<Vec<_>>(),
            "keyed": look.keys.tracks().tracks().count(),
        })
    }
}

pub(super) fn target_json(target: &Target) -> Json {
    match target {
        Target::Socket(key) => json!({
            "graph": graph_name(&key.graph),
            "node": key.socket.node.get(),
            "port": key.socket.port,
        }),
        Target::Slot(key) => json!({
            "figure": key.figure.as_str(),
            "slot": key.material,
            "parameter": key.parameter.as_str(),
        }),
    }
}

pub(super) fn curve_of(curve: VmdCurve) -> Json {
    animate_assistant::curve_name(curve).map_or_else(|| json!(curve.to_bytes()), Json::from)
}
