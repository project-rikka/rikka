use std::time::Duration;

use bitflags::bitflags;
use rikka_compositor_reactor::Timestamp;

use crate::framebuffer::Framebuffer;
use crate::ids::CrtcId;

#[derive(Debug)]
pub enum KmsEvent {
    Flipped {
        presented: Presented,
        retired: Option<Retired>,
    },
    Retired(Retired),
    Rejected {
        crtc: CrtcId,
        frame: u64,
        reason: Rejection,
    },
    CursorRejected {
        crtc: CrtcId,
    },
    Lost {
        crtc: CrtcId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presented {
    pub crtc: CrtcId,
    pub frame: u64,
    pub sequence: u64,
    pub at: Timestamp,
    pub refresh: Refresh,
    pub flags: PresentedFlags,
}

#[derive(Debug)]
pub struct Retired {
    pub crtc: CrtcId,
    pub frame: u64,
    pub shown: bool,
    pub primary: Framebuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refresh {
    Fixed(Duration),
    Variable { shortest: Duration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    Modeset,
    Scanout,
    Commit,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PresentedFlags: u8 {
        const VSYNC = 1 << 0;
        const HW_CLOCK = 1 << 1;
        const HW_COMPLETION = 1 << 2;
        const ZERO_COPY = 1 << 3;
    }
}
