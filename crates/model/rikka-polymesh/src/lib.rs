mod adapt;
mod attributes;
mod codec;
mod domain;
mod edge_index;
mod edit;
mod error;
mod gather;
mod health;
mod layers;
mod merge;
mod mesh_in;
mod mesh_out;
mod names;
mod normals;
mod polymesh;
mod shape_keys;
mod tangent;
mod topology;
mod transform;
mod triangulate;
mod validate;
mod values;
mod vertex_groups;
mod well_known;

pub use attributes::{Attribute, Attributes};
pub use domain::{Domain, DomainSizes};
pub use error::{Error, Result};
pub use health::Orientation;
pub use layers::Layers;
pub use merge::MergeData;
pub use mesh_in::FromMesh;
pub use mesh_out::{ColorSource, ConvertedMesh, ToMesh};
pub use names::{
    BEVEL_WEIGHT_EDGE, BEVEL_WEIGHT_VERT, CREASE_EDGE, CREASE_VERT, CUSTOM_NORMAL, HIDE_EDGE,
    HIDE_POLY, HIDE_VERT, MATERIAL_INDEX, SELECT_EDGE, SELECT_POLY, SELECT_VERT, SHARP_EDGE,
    SHARP_FACE, UV_PIN_PREFIX, UV_SEAM, UV_SELECT_EDGE, UV_SELECT_FACE, UV_SELECT_VERT,
    uv_pin_name,
};
pub use normals::{Shading, acos_approx, polygon_normal};
pub use polymesh::Polymesh;
pub use shape_keys::{ShapeKey, ShapeKeys};
pub use topology::{Components, EdgeKind, GroupedIndices};
pub use triangulate::{NgonMethod, QuadMethod, Triangulation};
pub use validate::ordered;
pub use values::{AttributeValue, DataType, Values};
pub use vertex_groups::{Influence, Influences, VertexGroup, VertexGroups};

const PARALLEL: rikka_parallel::Parallel = rikka_parallel::Parallel::new("rikka-polymesh");
