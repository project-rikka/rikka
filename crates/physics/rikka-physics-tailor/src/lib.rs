mod analysis;
mod asset;
mod bind;
mod body;
mod budget;
mod build;
mod cloth_settings;
mod constraints;
mod error;
mod figure;
mod frame;
mod geometry;
mod grab;
mod key;
mod layers;
mod pick;
mod preset;
mod proxy;
mod relief;
mod sheet;
mod strands;
mod style;
mod tuning;
mod underlay;
mod wearer;
mod wind;

const PARALLEL: rikka_parallel::Parallel = rikka_parallel::Parallel::new("rikka-tailor");

pub use analysis::{AnalyzedPiece, ClothAnalysis};
pub use asset::{
    Anchor, Bend, BindTarget, Body, BodyCollider, BodyShape, BodyVertex, ClothAsset, Edge,
    EdgeKind, Influence, MAX_INFLUENCES, MAX_STRAND_NODES, Particle, Piece, PieceKey, PieceKind,
    StrandBinding, StrandNode, StrandWeight, Tether, Underlay, VertexBinding,
};
pub use budget::{Budget, Quality};
pub use cloth_settings::{ClothSettings, Paint, PieceSettings, Role};
pub use error::{Error, Result};
pub use frame::{DrapedVertex, Frame, particle_normals};
pub use grab::{Grab, MAX_GRABS};
pub use key::VertexKey;
pub use pick::{ParticlePick, PickWeight};
pub use preset::Preset;
pub use style::{CURVE_SAMPLES, Curve, Flutter, Inertia, Style};
pub use tuning::ClothTuning;
pub use wearer::{Garment, GarmentVertex, Wearer};
pub use wind::Wind;
