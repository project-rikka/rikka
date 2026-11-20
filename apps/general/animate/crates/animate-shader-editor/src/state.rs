use std::cell::RefCell;
use std::sync::Arc;

use animate_look::{GraphRef, Look, Name};
use animate_shader::{Diagnostic, Workbench};
use rikka_app::prelude::*;
use rikka_shader_graph::{Fragment, NodeId, Scope};
use rikka_widgets::node_canvas::{CanvasMenu, NodeGraph, Selection};
use rustc_hash::FxHashMap;

use crate::canvas_data::node_id;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    World,
    Composite,
    Material,
}

impl Tab {
    pub const ALL: [Self; 3] = [Self::World, Self::Composite, Self::Material];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Renaming {
    Material(String),
    Function(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Popup {
    Materials,
    Scopes,
    Problems,
    AddParameter,
    Assign,
    Classes,
}

#[derive(Debug)]
struct Drawn {
    graph: GraphRef,
    scope: Scope,
    revision: u64,
    diagnostics: Arc<[Diagnostic]>,
    canvas: Arc<NodeGraph>,
}

#[derive(Debug, Default)]
pub struct ShaderEditor {
    pub(crate) tab: Tab,
    pub(crate) material: Option<String>,
    pub(crate) path: Vec<String>,
    pub(crate) selection: Selection,
    pub(crate) clipboard: Option<Fragment>,
    pub(crate) menu: Option<CanvasMenu>,
    pub(crate) query: TextDocument,
    pub(crate) catalog_pick: Option<usize>,
    pub(crate) naming: TextDocument,
    pub(crate) renaming: Option<Renaming>,
    pub(crate) popup: Option<Popup>,
    pub(crate) port_type: usize,
    pub(crate) notice: Option<String>,
    pub(crate) slot_figure: Option<Name>,
    pub(crate) slots: Vec<u32>,
    drawn: RefCell<Option<Drawn>>,
    fields: RefCell<FxHashMap<String, TextDocument>>,
}

impl ShaderEditor {
    pub fn tab(&self) -> Tab {
        self.tab
    }

    pub fn graph(&self) -> Option<GraphRef> {
        match self.tab {
            Tab::World => Some(GraphRef::World),
            Tab::Composite => Some(GraphRef::Composite),
            Tab::Material => self.material.clone().map(GraphRef::Material),
        }
    }

    pub fn scope(&self) -> Scope {
        self.path
            .last()
            .map_or(Scope::Main, |name| Scope::Function(name.clone()))
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn chosen_node(&self, look: &Look) -> Option<(GraphRef, NodeId)> {
        if !self.path.is_empty() {
            return None;
        }
        let at = self.graph()?;
        let body = look.graph(&at)?.scope(&Scope::Main)?;
        let mut nodes = self.selection.nodes().filter_map(|key| node_id(body, key));
        let id = nodes.next()?;
        nodes.next().is_none().then_some((at, id))
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn show(&mut self, graph: GraphRef) {
        let (tab, material) = match graph {
            GraphRef::World => (Tab::World, self.material.clone()),
            GraphRef::Composite => (Tab::Composite, self.material.clone()),
            GraphRef::Material(key) => (Tab::Material, Some(key)),
        };
        if tab == self.tab && material == self.material {
            return;
        }
        self.tab = tab;
        self.material = material;
        self.leave_to(0);
    }

    pub(crate) fn leave_to(&mut self, depth: usize) {
        self.path.truncate(depth);
        self.selection = Selection::default();
        self.menu = None;
        self.popup = None;
        self.renaming = None;
    }

    pub(crate) fn field(&self, id: &str, current: &str) -> TextDocument {
        let mut fields = self.fields.borrow_mut();
        let document = fields
            .entry(id.to_owned())
            .or_insert_with(|| TextDocument::new(current));
        if !document.is_modified() && document.text() != current {
            document.reset(current);
        }
        document.clone()
    }

    pub fn chosen_slots(&self, figure: Name) -> &[u32] {
        match self.slot_figure == Some(figure) {
            true => &self.slots,
            false => &[],
        }
    }

    pub(crate) fn canvas_graph(
        &self,
        look: &Look,
        workbench: &Workbench,
        diagnostics: &Arc<[Diagnostic]>,
    ) -> Option<Arc<NodeGraph>> {
        let at = self.graph()?;
        let scope = self.scope();
        let revision = workbench.graph_revision(&at)?;
        let mut drawn = self.drawn.borrow_mut();
        if let Some(drawn) = drawn.as_ref()
            && drawn.graph == at
            && drawn.scope == scope
            && drawn.revision == revision
            && drawn.diagnostics == *diagnostics
        {
            return Some(Arc::clone(&drawn.canvas));
        }
        let graph = look.graph(&at)?;
        let canvas = Arc::new(crate::canvas_data::node_graph(
            graph,
            workbench.library(),
            &scope,
            diagnostics,
        )?);
        *drawn = Some(Drawn {
            graph: at,
            scope,
            revision,
            diagnostics: Arc::clone(diagnostics),
            canvas: Arc::clone(&canvas),
        });
        Some(canvas)
    }
}
