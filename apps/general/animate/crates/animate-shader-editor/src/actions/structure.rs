use animate_look::{GraphRef, LookEdit, check_material_key};
use animate_shader::{empty_material, fresh_function};
use rikka_app::prelude::*;
use rikka_shader_graph::{Error as GraphError, Graph, Library};
use rikka_shader_graph::{Function, NodeId};
use rikka_shader_presets::Preset;
use rikka_widgets::node_canvas::{Item, Selection};

use crate::canvas_data::node_key;
use crate::state::{Renaming, Tab};
use crate::traits::Workshop;

impl dyn Workshop + '_ {
    pub(crate) fn show_tab(&mut self, tab: Tab) {
        match tab {
            Tab::World => self.shader_mut().show(GraphRef::World),
            Tab::Composite => self.shader_mut().show(GraphRef::Composite),
            Tab::Material => {
                self.settle_material();
                let shader = self.shader_mut();
                if shader.tab != Tab::Material {
                    shader.tab = Tab::Material;
                    shader.leave_to(0);
                }
            }
        }
    }

    pub(crate) fn show_material(&mut self, key: String) {
        self.shader_mut().show(GraphRef::Material(key));
    }

    fn settle_material(&mut self) {
        let materials = &self.look().materials;
        let kept = self
            .shader()
            .material
            .clone()
            .filter(|key| materials.contains_key(key));
        let chosen = kept.or_else(|| materials.keys().next().cloned());
        if chosen != self.shader().material {
            let shader = self.shader_mut();
            shader.material = chosen;
            if shader.tab == Tab::Material {
                shader.leave_to(0);
            }
        }
    }

    pub(crate) fn add_material(&mut self, starter: Starter) -> Option<String> {
        self.close_popup();
        let graph = match starter.graph(self.library()) {
            Ok(graph) => graph,
            Err(error) => {
                self.fail(error);
                return None;
            }
        };
        let added = self
            .workbench()
            .add_material(self.look(), starter.key(), graph);
        let (key, edit) = match added {
            Ok(added) => added,
            Err(error) => {
                self.fail(error);
                return None;
            }
        };
        self.apply_look(edit, None).then(|| {
            self.show_material(key.clone());
            key
        })
    }

    pub(crate) fn duplicate_material(&mut self) {
        self.close_popup();
        let Some(key) = self.shader().material.clone() else {
            return;
        };
        if let Some(copy) = self.copy_material(&key) {
            self.show_material(copy);
        }
    }

    pub(crate) fn copy_material(&mut self, key: &str) -> Option<String> {
        let copied = self.workbench().duplicate_material(self.look(), key);
        match copied {
            Ok((copy, edit)) => self.apply_look(edit, None).then_some(copy),
            Err(error) => {
                self.fail(error);
                None
            }
        }
    }

    pub(crate) fn remove_material(&mut self) {
        self.close_popup();
        let Some(key) = self.shader().material.clone() else {
            return;
        };
        if self.apply_look(LookEdit::RemoveMaterial { key }, None) {
            self.settle_material();
        }
    }

    pub(crate) fn start_renaming(&mut self, renaming: Renaming) {
        self.close_popup();
        let shader = self.shader_mut();
        let current = match &renaming {
            Renaming::Material(key) => key.clone(),
            Renaming::Function(name) => name.clone(),
        };
        shader.naming.reset(&current);
        shader.naming.select_all();
        shader.renaming = Some(renaming);
    }

    pub(crate) fn finish_renaming(&mut self) {
        let Some(renaming) = self.shader_mut().renaming.take() else {
            return;
        };
        let to = self.shader().naming.text().trim().to_owned();
        match renaming {
            Renaming::Material(from) => {
                if from == to {
                    return;
                }
                let renamed = LookEdit::RenameMaterial {
                    from: from.clone(),
                    to: to.clone(),
                };
                if self.apply_look(renamed, None)
                    && self.shader().material.as_deref() == Some(&from)
                {
                    self.shader_mut().material = Some(to);
                }
            }
            Renaming::Function(from) => self.rename_function(&from, &to),
        }
    }

    pub(crate) fn rename_function(&mut self, from: &str, to: &str) {
        if from == to {
            return;
        }
        if let Err(error) = check_material_key(to) {
            return self.fail(error);
        }
        let renamed = self.edit_graph(None, |graph, library| {
            animate_shader::rename_function(graph, library, from, to)
        });
        if renamed.is_some() {
            for step in &mut self.shader_mut().path {
                if step == from {
                    *step = to.to_owned();
                }
            }
        }
    }

    pub(crate) fn add_function(&mut self) {
        self.close_popup();
        let Some((_, graph)) = self.shown() else {
            return;
        };
        let name = fresh_function(graph, tr!("animate.shader.group_default").as_ref());
        if self
            .edit_graph(None, |graph, library| {
                graph.add_function(library, &name, Function::default())
            })
            .is_some()
        {
            self.enter(&name);
        }
    }

    pub(crate) fn remove_function(&mut self, name: &str) {
        self.close_popup();
        if let Some(depth) = self.shader().path.iter().position(|step| step == name) {
            self.leave_to(depth);
        }
        self.edit_graph(None, |graph, library| {
            animate_shader::remove_function(graph, library, name)
        });
    }

    pub(crate) fn collapse_selection(&mut self) {
        let nodes = self.selected_nodes();
        let Some((_, graph)) = self.shown() else {
            return;
        };
        if nodes.is_empty() {
            return;
        }
        let name = fresh_function(graph, tr!("animate.shader.group_default").as_ref());
        let scope = self.shader().scope();
        if let Some(call) = self.edit_graph(None, |graph, library| {
            animate_shader::collapse(graph, library, &scope, &nodes, &name)
        }) {
            self.select_node(call);
        }
    }

    pub(crate) fn expand_node(&mut self, id: NodeId) {
        let scope = self.shader().scope();
        if let Some(added) = self.edit_graph(None, |graph, library| {
            animate_shader::expand(graph, library, &scope, id)
        }) {
            self.select(Selection::of(
                added.into_iter().map(|id| Item::Node(node_key(id))),
            ));
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Starter {
    Empty,
    Preset(Preset),
}

impl Starter {
    pub(crate) fn graph(self, library: &Library) -> Result<Graph, GraphError> {
        match self {
            Self::Empty => empty_material(library),
            Self::Preset(preset) => preset.graph(library),
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Empty => "Material",
            Self::Preset(preset) => preset.title,
        }
    }
}
