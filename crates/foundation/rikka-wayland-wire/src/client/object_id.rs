use crate::key::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(u64);

impl ObjectId {
    pub(crate) fn new(id: u32, generation: u32) -> Self {
        Self((u64::from(generation) << 32) | u64::from(id))
    }
}

impl Key for ObjectId {
    fn into_raw(self) -> u64 {
        self.0
    }

    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}
