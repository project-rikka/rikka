mod any;
mod buffer;
mod capsule;
mod cuboid;
mod cylinder;
mod distance;
pub(crate) mod extrema;
mod hull;
mod manifold;
mod plane;
mod queries;
mod revolution;
mod rounding;
mod sphere;
mod support;

pub use any::Convex;
pub use buffer::{PROXY_BUFFER_POINTS, ProxyBuffer};
pub use capsule::Capsule;
pub use cuboid::Cuboid;
pub use cylinder::Cylinder;
pub use distance::{
    DistanceInput, DistanceOutput, SegmentDistance, ShapeCastPairInput, SimplexCache, Sweep,
    ToiInput, ToiOutput, ToiState, segment_distance, shape_cast_pair, shape_distance,
    time_of_impact,
};
pub use hull::{ConvexHull, HullEdge, HullHeader, MAX_HULL_VERTICES, SOA_ALIGNMENT};
pub(crate) use hull::{argmax, horizontal_argmax, rows, soa_stride};
pub use manifold::{
    BatchManifold, CapsuleLanes, CuboidLanes, PairCache, SatCache, SphereLanes, TransformLanes,
    collide, collide_batch_capsule_sphere, collide_batch_capsules, collide_batch_cuboid_sphere,
    collide_batch_spheres,
};
pub use plane::PLANE_EXTENT;
pub(crate) use plane::validate_plane;
pub use queries::{PointProjection, project_point};
pub use sphere::Sphere;
pub use support::{Support, SupportPoint};
