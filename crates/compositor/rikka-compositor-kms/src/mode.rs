use std::time::Duration;

use drm::control::{ModeFlags, ModeTypeFlags};

const FALLBACK_REFRESH_MILLIHERTZ: u32 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mode {
    raw: drm::control::Mode,
    refresh_millihertz: u32,
    refresh: Duration,
    vblank: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Timings {
    clock_khz: u64,
    vdisplay: u64,
    htotal: u64,
    vtotal: u64,
    vscan: u64,
    interlaced: bool,
    doublescan: bool,
}

impl Mode {
    pub(crate) fn new(raw: drm::control::Mode) -> Self {
        let timings = Timings::from_mode(&raw);
        let refresh_millihertz = timings.refresh_millihertz();
        let millihertz = match refresh_millihertz {
            0 => FALLBACK_REFRESH_MILLIHERTZ,
            millihertz => millihertz,
        };
        Self {
            raw,
            refresh_millihertz,
            refresh: Duration::from_nanos(1_000_000_000_000 / u64::from(millihertz)),
            vblank: timings.vblank(),
        }
    }

    pub(crate) fn raw(&self) -> &drm::control::Mode {
        &self.raw
    }

    pub fn width(&self) -> u32 {
        u32::from(self.raw.size().0)
    }

    pub fn height(&self) -> u32 {
        u32::from(self.raw.size().1)
    }

    pub fn refresh_millihertz(&self) -> u32 {
        self.refresh_millihertz
    }

    pub fn refresh(&self) -> Duration {
        self.refresh
    }

    pub fn preferred(&self) -> bool {
        self.raw.mode_type().contains(ModeTypeFlags::PREFERRED)
    }

    pub fn interlaced(&self) -> bool {
        self.raw.flags().contains(ModeFlags::INTERLACE)
    }

    pub(crate) fn vblank(&self) -> Duration {
        self.vblank
    }
}

impl Timings {
    fn from_mode(raw: &drm::control::Mode) -> Self {
        let flags = raw.flags();
        Self {
            clock_khz: u64::from(raw.clock()),
            vdisplay: u64::from(raw.size().1),
            htotal: u64::from(raw.hsync().2),
            vtotal: u64::from(raw.vsync().2),
            vscan: u64::from(raw.vscan()),
            interlaced: flags.contains(ModeFlags::INTERLACE),
            doublescan: flags.contains(ModeFlags::DBLSCAN),
        }
    }

    fn refresh_millihertz(&self) -> u32 {
        if self.htotal == 0 || self.vtotal == 0 {
            return 0;
        }
        let mut refresh =
            (self.clock_khz * 1_000_000 / self.htotal + self.vtotal / 2) / self.vtotal;
        if self.interlaced {
            refresh *= 2;
        }
        if self.doublescan {
            refresh /= 2;
        }
        if self.vscan > 1 {
            refresh /= self.vscan;
        }
        u32::try_from(refresh).unwrap_or(u32::MAX)
    }

    fn vblank(&self) -> Duration {
        if self.clock_khz == 0 {
            return Duration::ZERO;
        }
        let lines = self.vtotal.saturating_sub(self.vdisplay);
        Duration::from_nanos(lines * self.htotal * 1_000_000 / self.clock_khz)
    }
}
