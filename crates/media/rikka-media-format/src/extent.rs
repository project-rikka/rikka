#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
}
impl Extent {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Option<Extent> {
        if width == 0 || height == 0 {
            return None;
        }
        Some(Extent { width, height })
    }
}
