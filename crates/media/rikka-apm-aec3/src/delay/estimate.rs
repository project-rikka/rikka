#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Quality {
    Coarse,
    Refined,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DelayEstimate {
    pub(crate) quality: Quality,
    pub(crate) delay: usize,
    pub(crate) blocks_since_last_change: usize,
    pub(crate) blocks_since_last_update: usize,
}

impl DelayEstimate {
    pub(crate) fn new(quality: Quality, delay: usize) -> DelayEstimate {
        DelayEstimate {
            quality,
            delay,
            blocks_since_last_change: 0,
            blocks_since_last_update: 0,
        }
    }
}
