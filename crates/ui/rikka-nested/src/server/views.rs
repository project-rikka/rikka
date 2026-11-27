use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use rikka_compositor_geometry::{Logical, Point, Size};
use rikka_compositor_reactor::Token;
use rikka_compositor_shell::{PopupId, PopupParent, ShellRole, ToplevelConfigure, ToplevelId};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;
use slotmap::SecondaryMap;

use super::state::State;
use crate::claim::Claim;
use crate::shared::Shared;
use crate::view::ViewId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DesiredState {
    pub(crate) size: Option<Size<i32, Logical>>,
    pub(crate) scale: f64,
    pub(crate) focused: bool,
    pub(crate) visible: bool,
    pub(crate) fullscreen: bool,
}

impl Default for DesiredState {
    fn default() -> Self {
        Self {
            size: None,
            scale: 1.0,
            focused: false,
            visible: true,
            fullscreen: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Gate {
    pub(crate) serial: u32,
    pub(crate) patience: Token,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Placement {
    pub(crate) surface: SurfaceId,
    pub(crate) origin: Point<i32, Logical>,
}

pub(crate) struct ViewRecord {
    pub(crate) claim: Claim,
    pub(crate) pacing: OutputId,
    pub(crate) shared: Arc<Shared>,
    pub(crate) toplevel: Option<ToplevelId>,
    pub(crate) desired: DesiredState,
    pub(crate) sent: Option<ToplevelConfigure>,
    pub(crate) gate: Option<Gate>,
    pub(crate) dirty: bool,
    pub(crate) generation: u64,
    pub(crate) placed: Vec<Placement>,
    pub(crate) popups: Vec<PopupId>,
    pub(crate) grabs: Vec<PopupId>,
    pub(crate) frame_pending: bool,
    pub(crate) wake_pending: bool,
    pub(crate) last_presented: Option<Instant>,
}

impl ViewRecord {
    pub(crate) fn new(claim: Claim, pacing: OutputId, shared: Arc<Shared>) -> Self {
        Self {
            claim,
            pacing,
            shared,
            toplevel: None,
            desired: DesiredState::default(),
            sent: None,
            gate: None,
            dirty: false,
            generation: 0,
            placed: Vec::new(),
            popups: Vec::new(),
            grabs: Vec::new(),
            frame_pending: false,
            wake_pending: false,
            last_presented: None,
        }
    }

    pub(crate) fn origin_of(&self, surface: SurfaceId) -> Option<Point<i32, Logical>> {
        self.placed
            .iter()
            .find(|placement| placement.surface == surface)
            .map(|placement| placement.origin)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Tracked {
    pub(crate) view: Option<ViewId>,
    pub(crate) frame_pending: bool,
    pub(crate) announced: bool,
    pub(crate) owner: Option<ViewId>,
    pub(crate) adopted: Option<ToplevelId>,
}

#[derive(Default)]
pub(crate) struct Views {
    pub(crate) records: BTreeMap<ViewId, ViewRecord>,
    pub(crate) toplevels: SecondaryMap<ToplevelId, Tracked>,
}

impl State {
    pub(crate) fn toplevel_of(&self, surface: SurfaceId) -> Option<ToplevelId> {
        let root = self.surfaces.root(surface);
        match rikka_compositor_shell::shell_role(&self.surfaces, root)? {
            ShellRole::Toplevel(toplevel) => Some(toplevel),
            ShellRole::Popup(popup) => match self.shell.popup_root(popup)? {
                PopupParent::Toplevel(toplevel) => Some(toplevel),
                PopupParent::Popup(_) | PopupParent::Layer(_) => None,
            },
            ShellRole::Layer(_) | ShellRole::LockSurface(_) => None,
        }
    }

    pub(crate) fn view_of(&self, surface: SurfaceId) -> Option<ViewId> {
        let toplevel = self.toplevel_of(surface)?;
        self.owner_of(toplevel)
    }

    pub(crate) fn record(&mut self, view: ViewId) -> Option<&mut ViewRecord> {
        self.views.records.get_mut(&view)
    }

    pub(crate) fn mark_dirty(&mut self, view: ViewId) {
        if let Some(record) = self.record(view) {
            record.dirty = true;
        }
    }
}
