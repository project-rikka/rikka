use crate::outline::bounds::Bounds;
use crate::outline::sink::OutlineSink;

pub(crate) struct BoundingSink<'s, S> {
    inner: &'s mut S,
    pub(crate) bounds: Bounds,
}

impl<'s, S: OutlineSink> BoundingSink<'s, S> {
    pub(crate) fn new(inner: &'s mut S) -> Self {
        Self {
            inner,
            bounds: Bounds::EMPTY,
        }
    }
}

impl<S: OutlineSink> OutlineSink for BoundingSink<'_, S> {
    #[inline]
    fn move_to(&mut self, x: f32, y: f32) {
        self.bounds.include(x, y);
        self.inner.move_to(x, y);
    }

    #[inline]
    fn line_to(&mut self, x: f32, y: f32) {
        self.bounds.include(x, y);
        self.inner.line_to(x, y);
    }

    #[inline]
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.bounds.include(x1, y1);
        self.bounds.include(x, y);
        self.inner.quad_to(x1, y1, x, y);
    }

    #[inline]
    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.bounds.include(x1, y1);
        self.bounds.include(x2, y2);
        self.bounds.include(x, y);
        self.inner.cubic_to(x1, y1, x2, y2, x, y);
    }

    #[inline]
    fn close(&mut self) {
        self.inner.close();
    }
}
