use rustc_hash::FxHashMap;
use x11rb_protocol::protocol::xproto::Window;

use crate::properties::Fields;
use crate::window::{XWindow, XWindowId};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Titles {
    pub(crate) net: Option<String>,
    pub(crate) legacy: Option<String>,
}

impl Titles {
    pub(crate) fn current(&self) -> &str {
        self.net
            .as_deref()
            .or(self.legacy.as_deref())
            .unwrap_or_default()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Xdnd {
    pub(crate) version: Option<u32>,
    pub(crate) proxy: Option<Window>,
}

#[derive(Debug)]
pub(crate) struct Record {
    pub(crate) window: XWindow,
    pub(crate) titles: Titles,
    pub(crate) xdnd: Xdnd,
    pub(crate) epoch: u64,
    pub(crate) dirty: Fields,
    pub(crate) reading: u32,
}

impl Record {
    pub(crate) fn new(window: XWindow, epoch: u64) -> Self {
        Self {
            window,
            titles: Titles::default(),
            xdnd: Xdnd::default(),
            epoch,
            dirty: Fields::empty(),
            reading: 0,
        }
    }

    pub(crate) fn settled(&self) -> bool {
        self.dirty.is_empty() && self.reading == 0
    }
}

pub(crate) fn current(
    windows: &mut FxHashMap<XWindowId, Record>,
    window: XWindowId,
    epoch: u64,
) -> Option<&mut Record> {
    windows
        .get_mut(&window)
        .filter(|record| record.epoch == epoch)
}
