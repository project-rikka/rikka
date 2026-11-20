mod geometry;
mod manifold;

pub(crate) use geometry::RECYCLE_VALID;
pub use geometry::{
    ContactGeometry, ContactHeader, DISJOINT, ENABLE_RECYCLE, ExtraManifolds, PairKind,
    STARTED_TOUCHING, STATE_CHANGES, STOPPED_TOUCHING, TOUCHING,
};
pub use manifold::{ContactManifold, ContactPoint};
