mod adjacent;
mod arena;
mod beautify;
mod cloud;
mod compact;
mod convert;
mod core;
mod data;
mod edgeloop;
mod edgenet;
mod edgenet_islands;
mod elements;
mod error;
mod falloff;
mod geometry;
mod geometry_set;
mod groups;
mod heap;
mod ids;
mod interp;
mod join;
mod journal;
mod lnor;
pub mod math;
mod math_layers;
mod mesh;
mod mirror;
mod mods;
mod normals;
mod path;
mod primitives;
mod query;
mod random;
mod recalc;
mod region_match;
mod scanfill;
mod select;
mod select_ops;
mod separate;
mod sets;
mod similar;
mod split;
mod structure;
mod tessellate;
mod topology;
mod uv;
mod validate;
mod walk;
mod weld;

pub use crate::core::{DeleteContext, NewFace};
pub use beautify::{BeautifyFill, BeautyMethod};
pub use cloud::CloudNormal;
pub use compact::Compaction;
pub use edgeloop::EdgeLoop;
pub use error::{Error, Result};
pub use falloff::Falloff;
pub use geometry::FaceTangent;
pub use geometry_set::Geometry;
pub use heap::{Heap, HeapNode, SimpleHeap};
pub use ids::{EdgeId, Element, ElementId, ElementKind, FaceId, LoopId, VertId};
pub use interp::FaceSample;
pub use join::{DoubleFace, JoinOptions, JoinedFace};
pub use journal::{Changes, JournalMark};
pub use lnor::{
    EncodedCustomNormals, FanWalk, NormalSpace, NormalSpaces, corner_normals, edge_smooth_topology,
    is_cyclic_smooth_fan, walk_fan,
};
pub use math_layers::{MathLayer, MathLayerKind};
pub use mesh::{EditMesh, Flags, KeyBlock, Revisions, SelectMode};
pub use mirror::topology_mirror_table;
pub use mods::{Duplicate, RotateChecks, Rotation, Triangulated};
pub use path::{EdgePathTag, PathOptions};
pub use primitives::{AddedPrimitive, CircleFill, Primitive};
pub use random::Rng;
pub use rikka_polymesh::{DataType, Domain, NgonMethod, QuadMethod};
pub use scanfill::ScanFill;
pub use select_ops::{
    Axes, Checker, Compare, LinkedDelimit, NonManifold, PickAction, SelectAction,
};
pub use sets::{IdMap, IdSet, LocalSet, Marks};
pub use similar::{SimilarEdge, SimilarFace, SimilarRegions, SimilarValue, SimilarVert};
pub use split::{EdgeSplit, FacePathSplit, FaceSplit};
pub use tessellate::Tessellation;
pub use walk::{Walk, WalkDelimit};
pub use weld::WeldOptions;

const PARALLEL: rikka_parallel::Parallel =
    rikka_parallel::Parallel::new("rikka-mesh-edit").grain(1 << 14);
