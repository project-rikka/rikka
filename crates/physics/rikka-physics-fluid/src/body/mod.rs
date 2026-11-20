mod mesh;
mod morphs;
mod reach;
mod region;

pub(crate) use mesh::BodyMesh;
pub(crate) use reach::JointReach;
pub(crate) use region::Region;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionId(pub(crate) u32);
