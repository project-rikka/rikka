use rikka_compositor_types::FormatSet;

use crate::hdr::Signal;
use crate::ids::{ConnectorId, CrtcId};
use crate::mode::Mode;
use crate::swapchain::Swapchain;
use crate::timing::VblankTimeline;

#[derive(Debug)]
pub struct Output {
    pub(crate) crtc: CrtcId,
    pub(crate) connector: ConnectorId,
    pub(crate) name: String,
    pub(crate) mode: Mode,
    pub(crate) vrr: bool,
    pub(crate) vrr_capable: bool,
    pub(crate) max_bpc: Option<u32>,
    pub(crate) signal: Signal,
    pub(crate) swapchain: Swapchain,
    pub(crate) timeline: VblankTimeline,
    pub(crate) scanout_formats: FormatSet,
    pub(crate) cursor_plane: bool,
    pub(crate) async_flips: bool,
}

impl Output {
    pub fn crtc(&self) -> CrtcId {
        self.crtc
    }

    pub fn connector(&self) -> ConnectorId {
        self.connector
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn vrr(&self) -> bool {
        self.vrr
    }

    pub fn vrr_capable(&self) -> bool {
        self.vrr_capable
    }

    pub fn signal(&self) -> Signal {
        self.signal
    }

    pub fn swapchain(&self) -> &Swapchain {
        &self.swapchain
    }

    pub fn swapchain_mut(&mut self) -> &mut Swapchain {
        &mut self.swapchain
    }

    pub fn timeline(&self) -> &VblankTimeline {
        &self.timeline
    }

    pub fn scanout_formats(&self) -> &FormatSet {
        &self.scanout_formats
    }

    pub fn has_cursor_plane(&self) -> bool {
        self.cursor_plane
    }

    pub fn async_flips(&self) -> bool {
        self.async_flips
    }
}
