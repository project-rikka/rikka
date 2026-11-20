mod layout;

use std::collections::HashMap;

use rikka_compositor_desktop::{WindowId, Workspace};
use rikka_compositor_scrolling::{Frame, Sizing, Strip};
use rikka_compositor_types::OutputId;

pub(crate) use layout::{Kind, Layout};

#[derive(Debug)]
pub(crate) struct Tiling {
    kind: Kind,
    layouts: HashMap<Workspace, Layout>,
}

impl Tiling {
    pub(crate) fn new(kind: Kind) -> Self {
        Self {
            kind,
            layouts: HashMap::new(),
        }
    }

    pub(crate) fn kind(&self) -> Kind {
        self.kind
    }

    pub(crate) fn set_kind(&mut self, kind: Kind) {
        self.kind = kind;
    }

    pub(crate) fn rearranged(&mut self, kind: Kind) -> Vec<WindowId> {
        self.set_kind(kind);
        self.drain()
    }

    pub(crate) fn insert(
        &mut self,
        workspace: Workspace,
        frame: Frame,
        window: WindowId,
        next_to: Option<WindowId>,
    ) {
        let kind = self.kind;
        self.layouts
            .entry(workspace)
            .or_insert_with(|| Layout::new(kind, frame))
            .insert(frame, window, next_to);
    }

    pub(crate) fn remove(&mut self, window: WindowId) -> Option<Workspace> {
        let workspace = self.workspace_of(window)?;
        if let Some(layout) = self.layouts.get_mut(&workspace) {
            layout.remove(window);
            if layout.is_empty() {
                self.layouts.remove(&workspace);
            }
        }
        Some(workspace)
    }

    pub(crate) fn enlarge(&mut self, window: WindowId, sizing: Sizing) -> bool {
        self.workspace_of(window)
            .and_then(|workspace| self.layouts.get_mut(&workspace))
            .is_some_and(|layout| layout.enlarge(window, sizing))
    }

    pub(crate) fn workspace_of(&self, window: WindowId) -> Option<Workspace> {
        self.layouts
            .iter()
            .find(|(_, layout)| layout.contains(window))
            .map(|(workspace, _)| *workspace)
    }

    pub(crate) fn contains(&self, window: WindowId) -> bool {
        self.workspace_of(window).is_some()
    }

    pub(crate) fn scrolls(&self, window: WindowId) -> bool {
        self.layouts
            .values()
            .any(|layout| layout.strip().is_some_and(|strip| strip.contains(&window)))
    }

    pub(crate) fn layout(&self, workspace: Workspace) -> Option<&Layout> {
        self.layouts.get(&workspace)
    }

    pub(crate) fn on_mut(&mut self, workspace: Workspace) -> Option<&mut Layout> {
        self.layouts.get_mut(&workspace)
    }

    pub(crate) fn strip(&self, workspace: Workspace) -> Option<&Strip<WindowId>> {
        self.layouts.get(&workspace)?.strip()
    }

    pub(crate) fn strip_mut(&mut self, workspace: Workspace) -> Option<&mut Strip<WindowId>> {
        self.layouts.get_mut(&workspace)?.strip_mut()
    }

    pub(crate) fn tiled_representative(&self, window: WindowId) -> WindowId {
        self.layouts
            .values()
            .find_map(|layout| layout.strip().filter(|strip| strip.contains(&window)))
            .and_then(Strip::active)
            .unwrap_or(window)
    }

    pub(crate) fn drain(&mut self) -> Vec<WindowId> {
        std::mem::take(&mut self.layouts)
            .into_values()
            .flat_map(|layout| layout.windows())
            .collect()
    }

    pub(crate) fn forget(&mut self, output: OutputId) -> Vec<(usize, WindowId)> {
        let lost: Vec<Workspace> = self
            .layouts
            .keys()
            .filter(|workspace| workspace.output() == output)
            .copied()
            .collect();
        lost.into_iter()
            .filter_map(|workspace| {
                let layout = self.layouts.remove(&workspace)?;
                Some(
                    layout
                        .windows()
                        .into_iter()
                        .map(move |window| (workspace.number(), window)),
                )
            })
            .flatten()
            .collect()
    }
}
