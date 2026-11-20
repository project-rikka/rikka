use crate::scene::node::{GroupId, LayerId, NodeId};

pub(crate) type Dirt = u16;

pub(crate) const CONTENT: Dirt = 1;
pub(crate) const BOUNDS: Dirt = 2;
pub(crate) const ORDER: Dirt = 4;
pub(crate) const LAYER: Dirt = 8;
pub(crate) const KEPT: Dirt = 16;
pub(crate) const NEW: Dirt = 32;
pub(crate) const SHIFTED: Dirt = 64;
pub(crate) const SHOWN: Dirt = 128;
pub(crate) const MOVED: Dirt = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reset {
    Cleared,
    Restarted,
}

#[derive(Debug, Default)]
pub(crate) struct Journal {
    pub(crate) nodes: Vec<NodeId>,
    pub(crate) removed: Vec<NodeId>,
    pub(crate) groups: Vec<GroupId>,
    pub(crate) gone_groups: Vec<GroupId>,
    pub(crate) layers: Vec<LayerId>,
    pub(crate) gone_layers: Vec<LayerId>,
    pub(crate) moved_groups: Vec<u32>,
    pub(crate) reclipped_groups: Vec<u32>,
    pub(crate) restacked_layers: bool,
    pub(crate) faded_layers: bool,
    pub(crate) resequenced: bool,
    pub(crate) reset: Option<Reset>,
    pub(crate) background: bool,
    pub(crate) effects: bool,
}

impl Journal {
    pub(crate) fn clear(&mut self) {
        self.nodes.clear();
        self.removed.clear();
        self.groups.clear();
        self.gone_groups.clear();
        self.layers.clear();
        self.gone_layers.clear();
        self.moved_groups.clear();
        self.reclipped_groups.clear();
        self.restacked_layers = false;
        self.faded_layers = false;
        self.resequenced = false;
        self.reset = None;
        self.background = false;
        self.effects = false;
    }

    pub(crate) fn restart(&mut self, reset: Reset) {
        *self = Journal {
            reset: Some(match (self.reset, reset) {
                (Some(Reset::Cleared), _) => Reset::Cleared,
                _ => reset,
            }),
            background: self.background,
            effects: self.effects,
            ..Journal::default()
        };
    }

    pub(crate) fn has_frame_changes(&self) -> bool {
        !self.nodes.is_empty()
            || !self.removed.is_empty()
            || !self.groups.is_empty()
            || !self.gone_groups.is_empty()
            || !self.layers.is_empty()
            || !self.gone_layers.is_empty()
            || self.reset.is_some()
            || self.background
            || self.effects
    }

    pub(crate) fn has_batch_changes(&self) -> bool {
        !self.moved_groups.is_empty()
            || !self.reclipped_groups.is_empty()
            || self.restacked_layers
            || self.faded_layers
            || self.resequenced
    }

    pub(crate) fn is_empty(&self) -> bool {
        !self.has_frame_changes() && !self.has_batch_changes()
    }
}

#[derive(Debug, Default)]
pub(crate) struct Changes {
    pub(crate) nodes: Vec<(NodeId, Dirt)>,
    pub(crate) removed: Vec<NodeId>,
    pub(crate) moved_groups: Vec<u32>,
    pub(crate) reclipped_groups: Vec<u32>,
    pub(crate) restacked_layers: bool,
    pub(crate) faded_layers: bool,
    pub(crate) resequenced: bool,
    pub(crate) reset: bool,
}

impl Changes {
    pub(crate) fn clear(&mut self) {
        self.nodes.clear();
        self.removed.clear();
        self.moved_groups.clear();
        self.reclipped_groups.clear();
        self.restacked_layers = false;
        self.faded_layers = false;
        self.resequenced = false;
        self.reset = false;
    }
}
