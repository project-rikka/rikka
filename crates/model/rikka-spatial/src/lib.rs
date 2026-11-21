mod aabb;
mod bvh;
mod closest;
mod error;
mod identical;
mod kdtree;
mod mesh;
mod plane;
mod polygon;
mod predicates;
mod ray;
mod triangle;
mod view;

pub use crate::aabb::Aabb;
pub use crate::bvh::{Bvh, BvhBuilder, BvhUpdate, Candidate};
pub use crate::closest::{
    RaySegmentPair, SegmentPair, SegmentPoint, TrianglePoint, closest_point_on_segment,
    closest_point_on_triangle, closest_points_ray_segment, closest_points_segments,
};
pub use crate::error::{Error, Result};
pub use crate::identical::weld_identical;
pub use crate::kdtree::{Duplicates, PointNearest, PointTree};
pub use crate::mesh::{
    SegmentBvh, SegmentNearest, SegmentRayNearest, SegmentScreenNearest, TriangleBvh, TriangleHit,
    TriangleNearest, TrianglePair,
};
pub use crate::plane::{Containment, Frustum, Plane};
pub use crate::polygon::{
    FillRule, SegmentCrossing, SegmentIntersection2d, intersect_segments_2d, mean_value_weights,
    mean_value_weights_2d, point_in_polygon, polygon_area, polygon_normal, signed_area_2d,
    signed_mesh_volume, signed_volume, winding_number,
};
pub use crate::predicates::{orient2d, orient3d};
pub use crate::ray::{
    Culling, Ray, RayInterval, RayTriangleHit, WatertightRay, intersect_ray_aabb,
    intersect_ray_plane, intersect_ray_sphere, intersect_ray_triangle,
};
pub use crate::triangle::{
    TriangleIntersection, barycentric_weights, intersect_triangles, triangle_area,
    triangle_intersects_aabb, triangle_normal,
};
pub use crate::view::{ClipSpace, NdcDepth, NdcY, ScreenPoint, ScreenSegment, View, Viewport};
