mod broadphase;
mod codec;
mod collider;
mod contact;
mod continuous;
mod filter;
mod frame;
mod material;
mod mover;
mod narrowphase;
mod query;
mod sensor;

pub use broadphase::{Broadphase, ProxyKind, pair_key, pair_of};
pub use collider::Collider;
pub use contact::{
    ContactGeometry, ContactHeader, ContactManifold, ContactPoint, DISJOINT, ENABLE_RECYCLE,
    ExtraManifolds, PairKind, STARTED_TOUCHING, STATE_CHANGES, STOPPED_TOUCHING, TOUCHING,
};
pub use continuous::{SweptShape, shape_time_of_impact};
pub use filter::{Filter, PairFilter, QueryFilter};
pub use frame::{BodyFrame, rotation_arc};
pub use material::{Material, MixedMaterial};
pub use mover::{
    CollisionPlane, Mover, MoverPlane, PlaneSolution, clip_vector, collide_mover, solve_planes,
};
pub use narrowphase::{CollideInput, ManifoldPools, Narrowphase, PoolView};
pub use query::{PointHit, QueryHit, QueryWorld, Ray};
pub use sensor::{SensorEvent, SensorInput, Sensors, ShapeKey};

const MIN_WORKER_RESERVE: usize = 256;

#[inline]
pub(crate) fn worker_reserve(total: usize, workers: usize) -> usize {
    (total.div_ceil(workers) * 2)
        .max(MIN_WORKER_RESERVE)
        .min(total)
}
