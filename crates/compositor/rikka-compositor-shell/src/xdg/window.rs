use rikka_compositor_geometry::{Logical, Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct WindowState {
    pub(crate) geometry: Option<Rect<i32, Logical>>,
    pub(crate) min_size: Size<i32, Logical>,
    pub(crate) max_size: Size<i32, Logical>,
}

impl WindowState {
    pub(crate) fn limits_valid(&self) -> bool {
        let (min, max) = (self.min_size, self.max_size);
        (max.w == 0 || min.w <= max.w) && (max.h == 0 || min.h <= max.h)
    }
}
