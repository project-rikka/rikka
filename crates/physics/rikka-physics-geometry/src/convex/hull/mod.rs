mod blob;
mod box_features;
mod boxed;
mod bulk;
mod geometry;
mod makers;
mod quickhull;
mod rounded;
mod view;

pub use blob::{ConvexHull, HullHeader, MAX_HULL_VERTICES};
pub(crate) use box_features::{
    BOX_EDGE_AXES, BOX_FACE_LOOPS, box_edge, box_edge_corners, corner_point,
};
pub(crate) use boxed::{BOX_CORNERS, BOX_PAIR_FACES, BoxPolyhedron, corner_index};
pub use view::{HullEdge, SOA_ALIGNMENT};
pub(crate) use view::{Polyhedron, argmax, horizontal_argmax, rows, soa_stride};
