use std::collections::HashMap;

use rikka_compositor_server::DisplayHandle;
use rikka_compositor_surface::SurfaceId;
use rustc_hash::FxHashMap;
use slotmap::SlotMap;

use crate::clock::Clock;
use crate::foreign_list::Published;
use crate::host::ShellHost;
use crate::ids::{
    ActivationDraftId, IconDraftId, LayerId, PopupId, PositionerId, ToplevelDragId, ToplevelId,
    WmBaseId,
};
use crate::layer::LayerSurface;
use crate::lock::Locks;
use crate::xdg::{
    ActivationDraft, ActivationToken, Drag, Foreign, IconDraft, Popup, PositionerDraft, Toplevel,
    WmBase,
};

#[derive(Debug)]
pub struct Shell {
    pub(crate) clock: Clock,
    pub(crate) wm_bases: SlotMap<WmBaseId, WmBase>,
    pub(crate) positioners: SlotMap<PositionerId, PositionerDraft>,
    pub(crate) activations: SlotMap<ActivationDraftId, ActivationDraft>,
    pub(crate) icons: SlotMap<IconDraftId, IconDraft>,
    pub(crate) toplevels: SlotMap<ToplevelId, Toplevel>,
    pub(crate) popups: SlotMap<PopupId, Popup>,
    pub(crate) layers: SlotMap<LayerId, LayerSurface>,
    pub(crate) locks: Locks,
    pub(crate) tokens: HashMap<String, ActivationToken>,
    pub(crate) foreign: Foreign,
    pub(crate) drags: SlotMap<ToplevelDragId, Drag>,
    pub(crate) published: Published,
    pub(crate) xwayland: FxHashMap<u64, SurfaceId>,
}

impl Shell {
    pub fn new<S: ShellHost>(display: &DisplayHandle<S>) -> Self {
        Self {
            clock: Clock::new(display),
            wm_bases: SlotMap::with_key(),
            positioners: SlotMap::with_key(),
            activations: SlotMap::with_key(),
            icons: SlotMap::with_key(),
            toplevels: SlotMap::with_key(),
            popups: SlotMap::with_key(),
            layers: SlotMap::with_key(),
            locks: Locks::default(),
            tokens: HashMap::new(),
            foreign: Foreign::default(),
            drags: SlotMap::with_key(),
            published: Published::default(),
            xwayland: FxHashMap::default(),
        }
    }

    pub fn toplevel(&self, toplevel: ToplevelId) -> Option<&Toplevel> {
        self.toplevels.get(toplevel)
    }

    pub fn toplevels(&self) -> impl Iterator<Item = (ToplevelId, &Toplevel)> {
        self.toplevels.iter()
    }

    pub fn popup(&self, popup: PopupId) -> Option<&Popup> {
        self.popups.get(popup)
    }

    pub fn popups(&self) -> impl Iterator<Item = (PopupId, &Popup)> {
        self.popups.iter()
    }

    pub fn layer(&self, layer: LayerId) -> Option<&LayerSurface> {
        self.layers.get(layer)
    }

    pub fn layers(&self) -> impl Iterator<Item = (LayerId, &LayerSurface)> {
        self.layers.iter()
    }
}
