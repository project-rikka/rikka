use rikka_text_font::OutlineSink;
use rikka_types::Point;
use rikka_vector::Path;

#[derive(Clone, Debug, Default)]
pub struct Outline {
    path: Path,
}

impl Outline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.path.clear();
    }

    #[inline]
    pub fn is_truncated(&self) -> bool {
        self.path.is_truncated()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[inline]
    pub(crate) fn path_mut(&mut self) -> &mut Path {
        &mut self.path
    }
}

impl OutlineSink for Outline {
    #[inline]
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(Point::new(x, y));
    }

    #[inline]
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(Point::new(x, y));
    }

    #[inline]
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quad_to(Point::new(x1, y1), Point::new(x, y));
    }

    #[inline]
    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path
            .cubic_to(Point::new(x1, y1), Point::new(x2, y2), Point::new(x, y));
    }

    #[inline]
    fn close(&mut self) {}
}
