mod canvas;
mod slots;
pub(crate) mod structure;

pub(crate) use slots::users;
pub(crate) use structure::Starter;

use std::sync::Arc;

use animate_look::GraphRef;
use animate_shader::{Diagnostic, Merge, Status};
use rikka_shader_graph::{Change, Graph, GraphError, Library, NodeId, Scope};
use rikka_widgets::node_canvas::{CanvasMenu, Item, Selection};

use crate::canvas_data::node_key;
use crate::state::Popup;
use crate::traits::Workshop;

impl dyn Workshop + '_ {
    pub(crate) fn shown(&self) -> Option<(GraphRef, &Graph)> {
        let at = self.shader().graph()?;
        let graph = self.look().graph(&at)?;
        Some((at, graph))
    }

    pub(crate) fn library(&self) -> &Library {
        self.workbench().library()
    }

    pub(crate) fn fail(&mut self, error: impl std::fmt::Display) {
        self.shader_mut().notice = Some(error.to_string());
    }

    pub(crate) fn dismiss_notice(&mut self) {
        self.shader_mut().notice = None;
    }

    pub(crate) fn edit_graph<T>(
        &mut self,
        merge: Option<Merge>,
        edit: impl FnOnce(&mut Graph, &Library) -> Result<Change<T>, GraphError>,
    ) -> Option<T> {
        let at = self.shader().graph()?;
        let changed = self.workbench().change(self.look(), &at, edit);
        let (value, edit) = match changed {
            Ok(changed) => changed,
            Err(error) => {
                self.fail(error);
                return None;
            }
        };
        self.apply_look(edit, merge).then_some(value)
    }

    pub(crate) fn apply_look(
        &mut self,
        edit: animate_look::LookEdit,
        merge: Option<Merge>,
    ) -> bool {
        match self.edit_look(edit, merge) {
            Ok(()) => {
                self.shader_mut().notice = None;
                true
            }
            Err(error) => {
                self.fail(error);
                false
            }
        }
    }

    pub(crate) fn seal(&mut self) {
        self.seal_look();
    }

    pub(crate) fn status_of(&self, at: &GraphRef) -> Status {
        match at {
            GraphRef::Material(key) => self
                .shaders()
                .lock()
                .status(key)
                .unwrap_or(Status::Compiling),
            GraphRef::World => planned(self.workbench().world_plan(self.look()).map(|_| ())),
            GraphRef::Composite => {
                planned(self.workbench().composite_plan(self.look()).map(|_| ()))
            }
        }
    }

    pub(crate) fn diagnostics_of(&self, at: &GraphRef) -> Arc<[Diagnostic]> {
        match self.status_of(at) {
            Status::Failed(diagnostics) => diagnostics,
            Status::Compiling | Status::Ready => Arc::new([]),
        }
    }

    pub(crate) fn select(&mut self, selection: Selection) {
        let revealed = !selection.is_empty();
        self.shader_mut().selection = selection;
        if revealed {
            self.reveal_properties();
        }
    }

    pub(crate) fn select_node(&mut self, id: NodeId) {
        self.select(Selection::of([Item::Node(node_key(id))]));
    }

    pub(crate) fn toggle_popup(&mut self, popup: Popup) {
        let shader = self.shader_mut();
        shader.popup = match shader.popup == Some(popup) {
            true => None,
            false => Some(popup),
        };
    }

    pub(crate) fn close_popup(&mut self) {
        self.shader_mut().popup = None;
    }

    pub(crate) fn open_menu(&mut self, menu: CanvasMenu) {
        let shader = self.shader_mut();
        shader.menu = Some(menu);
        shader.catalog_pick = None;
        shader.query.reset("");
    }

    pub(crate) fn close_menu(&mut self) {
        self.shader_mut().menu = None;
    }

    pub(crate) fn pick_catalog(&mut self, pick: Option<usize>) {
        self.shader_mut().catalog_pick = pick;
    }

    pub(crate) fn enter(&mut self, function: &str) {
        if self
            .shown()
            .is_none_or(|(_, graph)| graph.function(function).is_none())
        {
            return;
        }
        let shader = self.shader_mut();
        let depth = shader.path.len();
        shader.leave_to(depth);
        shader.path.push(function.to_owned());
    }

    pub(crate) fn leave_to(&mut self, depth: usize) {
        self.shader_mut().leave_to(depth);
    }

    pub(crate) fn show_scope(&mut self, scope: &Scope) {
        match scope {
            Scope::Main => self.leave_to(0),
            Scope::Function(name) => {
                if self.shader().scope() != *scope {
                    self.leave_to(0);
                    self.enter(name);
                }
            }
        }
    }

    pub(crate) fn focus(&mut self, diagnostic: &Diagnostic) {
        self.close_popup();
        if let Some((scope, node)) = diagnostic.node() {
            let scope = scope.clone();
            self.show_scope(&scope);
            self.select_node(node);
        }
    }

    pub(crate) fn travel(&mut self, back: bool) {
        match back {
            true => self.undo(),
            false => self.redo(),
        }
        self.forget_missing();
    }

    pub(crate) fn forget_missing(&mut self) {
        let Some((_, graph)) = self.shown() else {
            return;
        };
        let scope = self.shader().scope();
        let Some(body) = graph.scope(&scope) else {
            self.leave_to(0);
            return;
        };
        let kept = self.shader().selection.retained(|item| match item {
            Item::Node(key) => crate::canvas_data::node_id(body, key).is_some(),
            Item::Frame(key) => crate::canvas_data::frame_id(body, key).is_some(),
            Item::Link(_) => false,
        });
        self.select(kept);
    }
}

fn planned(plan: Result<(), Arc<[Diagnostic]>>) -> Status {
    match plan {
        Ok(()) => Status::Ready,
        Err(diagnostics) => Status::Failed(diagnostics),
    }
}
