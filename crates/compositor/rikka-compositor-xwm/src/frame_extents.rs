#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FrameExtents {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

impl FrameExtents {
    pub(crate) fn cardinals(self) -> [u32; 4] {
        [self.left, self.right, self.top, self.bottom]
    }
}
