#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceId(pub(crate) u32);

impl FaceId {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
