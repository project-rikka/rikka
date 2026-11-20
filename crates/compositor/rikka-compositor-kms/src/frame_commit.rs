use std::os::fd::RawFd;

use rikka_compositor_types::Format;

use crate::atomic::Sink;
use crate::properties::{ConnectorProps, CrtcProps, PlaneProps, Signalling};
use crate::rect::Rect;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameCommit {
    pub(crate) crtc: CrtcProps,
    pub(crate) modeset: Option<Modeset>,
    pub(crate) vrr: Option<bool>,
    pub(crate) primary: Option<PlaneUpdate>,
    pub(crate) overlay: Option<PlaneUpdate>,
    pub(crate) cursor: Option<PlaneUpdate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Modeset {
    pub(crate) connector: ConnectorProps,
    pub(crate) mode_blob: u64,
    pub(crate) max_bpc: Option<u64>,
    pub(crate) signalling: Signalling,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PlaneUpdate {
    Show(PlaneScanout),
    Hide(PlaneProps),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlaneScanout {
    pub(crate) plane: PlaneProps,
    pub(crate) framebuffer: u32,
    pub(crate) format: Format,
    pub(crate) source: Rect,
    pub(crate) destination: Rect,
    pub(crate) fence: Option<RawFd>,
    pub(crate) damage: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Config {
    modeset: Option<Modeset>,
    vrr: Option<bool>,
    primary: Option<PlaneConfig>,
    overlay: Option<PlaneConfig>,
    cursor: Option<PlaneConfig>,
    asynchronous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaneConfig {
    Hidden,
    PlaneScanout {
        format: Format,
        source: Rect,
        size: (u32, u32),
    },
}

impl FrameCommit {
    pub(crate) fn new(crtc: CrtcProps) -> Self {
        Self {
            crtc,
            modeset: None,
            vrr: None,
            primary: None,
            overlay: None,
            cursor: None,
        }
    }

    pub(crate) fn is_modeset(&self) -> bool {
        self.modeset.is_some()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.modeset.is_none()
            && self.vrr.is_none()
            && self.primary.is_none()
            && self.overlay.is_none()
            && self.cursor.is_none()
    }

    pub(crate) fn changes_vrr(&self) -> bool {
        self.vrr.is_some() && self.modeset.is_none()
    }

    pub(crate) fn without_cursor(&self) -> Option<Self> {
        let rest = Self {
            cursor: None,
            ..*self
        };
        (self.cursor.is_some() && !rest.is_empty()).then_some(rest)
    }

    pub(crate) fn config(&self, asynchronous: bool) -> Config {
        Config {
            modeset: self.modeset,
            vrr: self.vrr,
            primary: self.primary.map(PlaneUpdate::config),
            overlay: self.overlay.map(PlaneUpdate::config),
            cursor: self.cursor.map(PlaneUpdate::config),
            asynchronous,
        }
    }

    pub(crate) fn encode(&self, sink: &mut impl Sink) {
        let crtc = self.crtc;
        if let Some(modeset) = &self.modeset {
            sink.set(crtc.id, crtc.active, 1);
            sink.set(crtc.id, crtc.mode_id, modeset.mode_blob);
        }
        if let (Some(enabled), Some(property)) = (self.vrr, crtc.vrr_enabled) {
            sink.set(crtc.id, property, u64::from(enabled));
        }
        if let Some(modeset) = &self.modeset {
            let connector = modeset.connector;
            sink.set(connector.id, connector.crtc_id, u64::from(crtc.id));
            if let (Some(bpc), Some(property)) = (modeset.max_bpc, connector.max_bpc) {
                sink.set(connector.id, property, bpc);
            }
            connector.encode_signalling(modeset.signalling, sink);
        }
        for update in [self.primary, self.overlay, self.cursor]
            .into_iter()
            .flatten()
        {
            update.encode(crtc.id, sink);
        }
    }
}

impl PlaneUpdate {
    fn config(self) -> PlaneConfig {
        match self {
            Self::Hide(_) => PlaneConfig::Hidden,
            Self::Show(shown) => PlaneConfig::PlaneScanout {
                format: shown.format,
                source: shown.source,
                size: (shown.destination.width, shown.destination.height),
            },
        }
    }

    fn encode(self, crtc: u32, sink: &mut impl Sink) {
        match self {
            Self::Hide(plane) => {
                sink.set(plane.id, plane.fb_id, 0);
                sink.set(plane.id, plane.crtc_id, 0);
            }
            Self::Show(shown) => shown.encode(crtc, sink),
        }
    }
}

impl PlaneScanout {
    fn encode(&self, crtc: u32, sink: &mut impl Sink) {
        let plane = self.plane;
        let [src_x, src_y, src_w, src_h] = plane.source;
        let [crtc_x, crtc_y, crtc_w, crtc_h] = plane.destination;
        sink.set(plane.id, plane.fb_id, u64::from(self.framebuffer));
        sink.set(plane.id, plane.crtc_id, u64::from(crtc));
        sink.set(plane.id, src_x, fixed(self.source.x.max(0).unsigned_abs()));
        sink.set(plane.id, src_y, fixed(self.source.y.max(0).unsigned_abs()));
        sink.set(plane.id, src_w, fixed(self.source.width));
        sink.set(plane.id, src_h, fixed(self.source.height));
        sink.set(plane.id, crtc_x, signed(self.destination.x));
        sink.set(plane.id, crtc_y, signed(self.destination.y));
        sink.set(plane.id, crtc_w, u64::from(self.destination.width));
        sink.set(plane.id, crtc_h, u64::from(self.destination.height));
        if let (Some(fence), Some(property)) = (self.fence, plane.in_fence) {
            sink.set(plane.id, property, signed(fence));
        }
        if let (Some(blob), Some(property)) = (self.damage, plane.damage_clips) {
            sink.set(plane.id, property, blob);
        }
        if let Some(blending) = plane.blending {
            blending.encode(plane.id, sink);
        }
    }
}

fn fixed(pixels: u32) -> u64 {
    u64::from(pixels) << 16
}

fn signed(value: i32) -> u64 {
    i64::from(value) as u64
}
