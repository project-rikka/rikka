use rikka_compositor_geometry::{Logical, Rect, Transform};
use rikka_compositor_types::OutputLayout;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Output {
    pub(crate) layout: OutputLayout,
    pub(crate) work_area: Rect<i32, Logical>,
    pub(crate) showing: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OutputState {
    pub(crate) work_area: Rect<i32, Logical>,
    pub(crate) showing: usize,
}

impl Output {
    pub fn layout(&self) -> OutputLayout {
        self.layout
    }

    pub fn geometry(&self) -> Rect<i32, Logical> {
        self.layout.geometry
    }

    pub fn scale(&self) -> f64 {
        self.layout.scale
    }

    pub fn transform(&self) -> Transform {
        self.layout.transform
    }

    pub fn work_area(&self) -> Rect<i32, Logical> {
        self.work_area
    }

    pub fn showing(&self) -> usize {
        self.showing
    }
}

impl OutputState {
    pub(crate) fn new(layout: OutputLayout) -> Self {
        Self {
            work_area: layout.geometry,
            showing: 1,
        }
    }

    pub(crate) fn with(self, layout: OutputLayout) -> Output {
        Output {
            layout,
            work_area: self.work_area,
            showing: self.showing,
        }
    }
}
