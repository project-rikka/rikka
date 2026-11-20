use rikka_compositor_geometry::{Buffer, Physical, Rect, Size, Transform};
use rikka_compositor_types::{Format, TearingHint};

#[derive(Debug, Clone, PartialEq)]
pub struct Surface<S> {
    pub id: S,
    pub format: Format,
    pub buffer: Size<i32, Buffer>,
    pub source: Rect<f64, Buffer>,
    pub transform: Transform,
    pub placement: Rect<i32, Physical>,
    pub opaque: bool,
    pub covered: bool,
    pub tearing: TearingHint,
}

impl<S> Surface<S> {
    pub(crate) fn whole_buffer(&self) -> Rect<f64, Buffer> {
        Rect::from_size(self.buffer).to_f64()
    }
}
