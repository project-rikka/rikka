use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use rustix::event::{EventfdFlags, eventfd};

use crate::ids::CrtcId;

#[derive(Debug)]
pub(crate) struct CursorSlots {
    bell: OwnedFd,
    open: AtomicBool,
    current: AtomicU32,
    slots: Box<[Slot]>,
}

#[derive(Debug)]
#[repr(align(64))]
struct Slot {
    crtc: u32,
    at: AtomicU64,
    pending: AtomicBool,
}

impl CursorSlots {
    pub(crate) fn new(crtcs: impl IntoIterator<Item = u32>) -> io::Result<Self> {
        Ok(Self {
            bell: eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?,
            open: AtomicBool::new(true),
            current: AtomicU32::new(0),
            slots: crtcs
                .into_iter()
                .map(|crtc| Slot {
                    crtc,
                    at: AtomicU64::new(0),
                    pending: AtomicBool::new(false),
                })
                .collect(),
        })
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    pub(crate) fn close(&self) {
        self.open.store(false, Ordering::Release);
    }

    pub(crate) fn move_to(&self, crtc: CrtcId, x: i32, y: i32) {
        if self.place(crtc, x, y) {
            let _ = rustix::io::write(&self.bell, &1u64.to_ne_bytes());
        }
    }

    pub(crate) fn drain(&self) {
        let mut count = [0u8; 8];
        let _ = rustix::io::read(&self.bell, &mut count);
    }

    pub(crate) fn pending(&self, crtc: CrtcId) -> bool {
        self.slot(crtc)
            .is_some_and(|slot| slot.pending.load(Ordering::Acquire))
    }

    pub(crate) fn take(&self, crtc: CrtcId) -> Option<Option<(i32, i32)>> {
        let slot = self.slot(crtc)?;
        slot.pending
            .swap(false, Ordering::AcqRel)
            .then(|| self.position(crtc))
    }

    pub(crate) fn position(&self, crtc: CrtcId) -> Option<(i32, i32)> {
        let slot = self.slot(crtc)?;
        (self.current.load(Ordering::Acquire) == slot.crtc)
            .then(|| unpack(slot.at.load(Ordering::Acquire)))
    }

    fn place(&self, crtc: CrtcId, x: i32, y: i32) -> bool {
        if let Some(slot) = self.slot(crtc) {
            slot.at.store(pack(x, y), Ordering::Release);
        }
        let left = self.current.swap(crtc.raw(), Ordering::AcqRel);
        let mut ring = false;
        if left != crtc.raw()
            && let Some(slot) = self.slots.iter().find(|slot| slot.crtc == left)
        {
            ring |= !slot.pending.swap(true, Ordering::AcqRel);
        }
        if let Some(slot) = self.slot(crtc) {
            ring |= !slot.pending.swap(true, Ordering::AcqRel);
        }
        ring
    }

    fn slot(&self, crtc: CrtcId) -> Option<&Slot> {
        self.slots.iter().find(|slot| slot.crtc == crtc.raw())
    }
}

impl AsFd for CursorSlots {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.bell.as_fd()
    }
}

fn pack(x: i32, y: i32) -> u64 {
    (u64::from(x.cast_unsigned()) << 32) | u64::from(y.cast_unsigned())
}

fn unpack(at: u64) -> (i32, i32) {
    let [high, low] = [(at >> 32) as u32, at as u32];
    (high.cast_signed(), low.cast_signed())
}
