use std::os::fd::OwnedFd;

use rikka_compositor_reactor::Timestamp;
use rikka_compositor_types::Flip;

use crate::damage::Damage;
use crate::framebuffer::{Framebuffer, Origin};
use crate::ids::CrtcId;
use crate::rect::Rect;

#[derive(Debug)]
#[must_use]
pub struct Present {
    pub(crate) crtc: CrtcId,
    pub(crate) frame: u64,
    pub(crate) primary: Framebuffer,
    pub(crate) fence: Option<Fence>,
    pub(crate) damage: Damage,
    pub(crate) not_before: Option<Timestamp>,
    pub(crate) flip: Flip,
}

#[derive(Debug)]
pub(crate) enum Fence {
    SyncFile(OwnedFd),
    Readable(OwnedFd),
}

impl Fence {
    pub(crate) fn fd(&self) -> &OwnedFd {
        match self {
            Self::SyncFile(fd) | Self::Readable(fd) => fd,
        }
    }

    pub(crate) fn sync_file(&self) -> Option<&OwnedFd> {
        match self {
            Self::SyncFile(fd) => Some(fd),
            Self::Readable(_) => None,
        }
    }
}

impl Present {
    pub fn new(crtc: CrtcId, frame: u64, primary: Framebuffer) -> Self {
        Self {
            crtc,
            frame,
            primary,
            fence: None,
            damage: Damage::default(),
            not_before: None,
            flip: Flip::Vsync,
        }
    }

    pub fn fence(mut self, sync_file: OwnedFd) -> Self {
        self.fence = Some(Fence::SyncFile(sync_file));
        self
    }

    pub fn ready_when_readable(mut self, readable: OwnedFd) -> Self {
        self.fence = Some(Fence::Readable(readable));
        self
    }

    pub fn damage(mut self, damage: impl IntoIterator<Item = Rect>) -> Self {
        for rect in damage {
            self.damage.add(rect);
        }
        self
    }

    pub fn not_before(mut self, at: Timestamp) -> Self {
        self.not_before = Some(at);
        self
    }

    pub fn flip(mut self, flip: Flip) -> Self {
        self.flip = flip;
        self
    }

    pub(crate) fn zero_copy(&self) -> bool {
        self.primary.origin() == Origin::Client
    }
}
