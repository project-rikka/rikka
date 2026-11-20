use crate::window::WindowFunction;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SincParams {
    pub(crate) taps: usize,
    pub(crate) window: WindowFunction,
    pub(crate) oversampling: usize,
}

impl SincParams {
    pub(crate) fn new(taps: usize, window: WindowFunction) -> SincParams {
        SincParams {
            taps,
            window,
            oversampling: 256,
        }
    }

    pub(crate) fn oversampling(mut self, oversampling: usize) -> SincParams {
        self.oversampling = oversampling;
        self
    }
}
