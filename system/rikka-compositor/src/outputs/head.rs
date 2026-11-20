use std::time::Duration;

use crate::color::HdrDisplay;

pub(crate) const FALLBACK_REFRESH: u32 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Mode {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) refresh: u32,
}

impl Mode {
    pub(crate) fn interval(self) -> Duration {
        interval(self.refresh)
    }
}

pub(crate) fn interval(millihertz: u32) -> Duration {
    let millihertz = match millihertz {
        0 => FALLBACK_REFRESH,
        refresh => refresh,
    };
    Duration::from_nanos(1_000_000_000_000 / u64::from(millihertz))
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Head {
    pub(crate) name: String,
    pub(crate) make: String,
    pub(crate) model: String,
    pub(crate) serial: Option<String>,
    pub(crate) size_mm: (u32, u32),
    pub(crate) modes: Vec<Mode>,
    pub(crate) scale: f64,
    pub(crate) hdr: Option<HdrDisplay>,
}
