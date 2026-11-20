#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GlyphImageKind {
    #[default]
    Mask,
    Color,
}

impl GlyphImageKind {
    #[inline]
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Mask => 1,
            Self::Color => 4,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GlyphImage {
    kind: GlyphImageKind,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Placement {
    #[inline]
    pub(crate) fn area(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

impl GlyphImage {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn kind(&self) -> GlyphImageKind {
        self.kind
    }

    #[inline]
    pub fn left(&self) -> i32 {
        self.left
    }

    #[inline]
    pub fn top(&self) -> i32 {
        self.top
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    #[inline]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    #[inline]
    pub fn row(&self, y: u32) -> &[u8] {
        let row = self.width as usize * self.kind.bytes_per_pixel();
        &self.pixels[y as usize * row..(y as usize + 1) * row]
    }

    pub(crate) fn prepare(&mut self, kind: GlyphImageKind, placement: Placement) -> &mut [u8] {
        self.pixels.clear();
        self.prepare_overwritten(kind, placement)
    }

    pub(crate) fn prepare_overwritten(
        &mut self,
        kind: GlyphImageKind,
        placement: Placement,
    ) -> &mut [u8] {
        self.kind = kind;
        self.left = placement.left;
        self.top = placement.top;
        self.width = placement.width;
        self.height = placement.height;
        self.pixels
            .resize(placement.area() * kind.bytes_per_pixel(), 0);
        &mut self.pixels
    }

    pub(crate) fn clear(&mut self, kind: GlyphImageKind) {
        self.prepare(kind, Placement::default());
    }
}
