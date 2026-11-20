#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DelayAdjustment {
    None,
    BufferFlush,
    NewDetectedDelay,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EchoPathVariability {
    pub(crate) gain_change: bool,
    pub(crate) delay_change: DelayAdjustment,
    pub(crate) clock_drift: bool,
}

impl EchoPathVariability {
    pub(crate) fn delay_changed(&self) -> bool {
        self.delay_change != DelayAdjustment::None
    }

    pub(crate) fn audio_path_changed(&self) -> bool {
        self.gain_change || self.delay_changed()
    }
}
