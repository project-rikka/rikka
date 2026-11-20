use crate::hdr::Signal;
use crate::mode::Mode;

#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct OutputConfig {
    pub(crate) mode: Mode,
    pub(crate) vrr: bool,
    pub(crate) max_bpc: Option<u32>,
    pub(crate) signal: Signal,
}

impl OutputConfig {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            vrr: false,
            max_bpc: None,
            signal: Signal::Sdr,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn is_hdr(&self) -> bool {
        matches!(self.signal, Signal::Pq(_))
    }

    pub fn vrr(mut self, enabled: bool) -> Self {
        self.vrr = enabled;
        self
    }

    pub fn max_bpc(mut self, bits: u32) -> Self {
        self.max_bpc = Some(bits);
        self
    }

    pub fn signal(mut self, signal: Signal) -> Self {
        self.signal = signal;
        self
    }
}
