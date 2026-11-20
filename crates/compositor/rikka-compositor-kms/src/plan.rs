use std::collections::BTreeMap;

use drm::control::{AtomicCommitFlags, property};

use crate::atomic::Sink;
use crate::frame_commit::FrameCommit;
use crate::properties::{ConnectorProps, CrtcProps, PlaneProps, Signalling};

#[derive(Debug, Clone, Default)]
pub(crate) struct Plan {
    changes: BTreeMap<(u32, u32), (property::Handle, u64)>,
    modeset: bool,
}

impl Plan {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn switch_off(&mut self, crtc: &CrtcProps) {
        self.set(crtc.id, crtc.active, 0);
        self.set(crtc.id, crtc.mode_id, 0);
        self.modeset = true;
    }

    pub(crate) fn detach(&mut self, connector: &ConnectorProps) {
        self.set(connector.id, connector.crtc_id, 0);
        connector.encode_signalling(Signalling::Sdr, self);
        self.modeset = true;
    }

    pub(crate) fn hide(&mut self, plane: &PlaneProps) {
        self.set(plane.id, plane.fb_id, 0);
        self.set(plane.id, plane.crtc_id, 0);
    }

    pub(crate) fn apply(&mut self, commit: &FrameCommit) {
        commit.encode(self);
        self.modeset |= commit.is_modeset();
    }

    pub(crate) fn flags(&self) -> AtomicCommitFlags {
        match self.modeset {
            true => AtomicCommitFlags::ALLOW_MODESET,
            false => AtomicCommitFlags::empty(),
        }
    }

    pub(crate) fn encode(&self, sink: &mut impl Sink) {
        for ((object, _), (property, value)) in &self.changes {
            sink.set(*object, *property, *value);
        }
    }
}

impl Sink for Plan {
    fn set(&mut self, object: u32, property: property::Handle, value: u64) {
        self.changes
            .insert((object, u32::from(property)), (property, value));
    }
}
