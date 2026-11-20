mod cleanup;
mod common;
mod decimate;
mod degenerate;
mod delete;
mod dissolve;
mod doubles;
mod error;
mod euler;
mod intersect;
mod join_triangles;
mod layers;
mod limited;
mod math;
mod merge;
mod quadric;
mod unsubdivide;
mod weld;

pub use cleanup::{collapse_short_edges, remove_duplicate_faces};
pub use common::Removed;
pub use decimate::{Axis, Collapse, DecimateOptions, decimate, decimate_collapse};
pub use degenerate::{
    DissolveDegenerateOptions, dissolve_degenerate, dissolve_degenerate_geometry,
};
pub use delete::{DeleteLooseOptions, DeleteOptions, DeleteType, delete, delete_loose};
pub use dissolve::{
    DeleteEdgeLoopOptions, DissolveEdgesOptions, DissolveFacesOptions, DissolveModeOptions,
    DissolveOutput, DissolveVertsOptions, delete_edge_loop, dissolve_edges,
    dissolve_edges_geometry, dissolve_faces, dissolve_faces_geometry, dissolve_mode,
    dissolve_verts, dissolve_verts_geometry,
};
pub use doubles::{
    AutoMergeOptions, FindDoublesOptions, RemoveDoublesOptions, auto_merge, find_doubles,
    remove_doubles,
};
pub use error::{Error, Result};
pub use intersect::{auto_merge_split, intersect_edges};
pub use join_triangles::{
    JoinTrianglesOptions, JoinTrianglesSettings, join_triangles, join_triangles_geometry,
};
pub use limited::{
    Delimit, LimitedDissolveOptions, decimate_planar, dissolve_limited, dissolve_limited_geometry,
};
pub use merge::{
    MergeOptions, MergeType, average_face_data, collapse_edges, collapse_geometry, merge,
    merge_face_data_at, point_merge,
};
pub use unsubdivide::{
    UnsubdivideOptions, decimate_unsubdivide, unsubdivide, unsubdivide_geometry,
};
pub use weld::{WeldOptions, weld};
