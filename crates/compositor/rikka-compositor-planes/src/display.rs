use rikka_compositor_geometry::{Physical, Rect, Size, Transform};
use rikka_compositor_types::FormatSet;

use crate::tearing::Tearing;

#[derive(Debug, Clone, Copy)]
pub struct Display<'a> {
    mode: Size<i32, Physical>,
    transform: Transform,
    primary: &'a FormatSet,
    tearing: Tearing,
}

impl<'a> Display<'a> {
    pub fn new(mode: Size<i32, Physical>, transform: Transform, primary: &'a FormatSet) -> Self {
        Self {
            mode,
            transform,
            primary,
            tearing: Tearing::Forbidden,
        }
    }

    #[must_use]
    pub fn with_tearing(mut self, tearing: Tearing) -> Self {
        self.tearing = tearing;
        self
    }

    pub(crate) fn formats(&self) -> &'a FormatSet {
        self.primary
    }

    pub(crate) fn mode(&self) -> Size<i32, Physical> {
        self.mode
    }

    pub(crate) fn transform(&self) -> Transform {
        self.transform
    }

    pub(crate) fn tearing(&self) -> Tearing {
        self.tearing
    }

    pub fn whole(&self) -> Rect<i32, Physical> {
        Rect::from_size(self.mode.transformed(self.transform))
    }
}
