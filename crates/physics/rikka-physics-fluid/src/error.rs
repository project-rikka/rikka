use rikka_model_core::NodeId;

use crate::body::BodyId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("fluid GPU operation failed: {0}")]
    Gpu(#[from] rikka_gpu::Error),

    #[error("{0} must be finite")]
    NonFinite(&'static str),

    #[error("particle radius {0} m out of range {min}..={max}", min = crate::builder::MIN_RADIUS, max = crate::builder::MAX_RADIUS)]
    RadiusOutOfRange(f32),

    #[error("particle capacity {0} out of range 1..{max}", max = rikka_physics_gpu::MAX_SCANNED)]
    CapacityOutOfRange(u32),

    #[error("spray capacity {0} must be below {max}", max = rikka_physics_gpu::MAX_SCANNED)]
    SprayCapacityOutOfRange(u32),

    #[error("fluid needs at least one iteration and a non-empty substep range")]
    InvalidIterations,

    #[error("fluid needs 1..={max} substances, got {0}", max = crate::substance::MAX_SUBSTANCES)]
    SubstanceCount(usize),

    #[error("model cannot be a fluid body: {0}")]
    NotABody(String),

    #[error("no fluid body {0:?}")]
    UnknownBody(BodyId),

    #[error("{body:?} has no mesh at node {}", node.index())]
    MissingMesh { body: BodyId, node: NodeId },

    #[error("mesh at node {} has {triangles} triangles, triangle {triangle} requested", node.index())]
    TriangleOutOfRange {
        node: NodeId,
        triangle: u32,
        triangles: u32,
    },

    #[error("region needs at least one triangle")]
    EmptyRegion,

    #[error("{count} body triangles exceed limit {limit}")]
    TooManyBodyTriangles { count: u32, limit: u32 },

    #[error("{count} body vertices exceed limit {limit}")]
    TooManyBodyVertices { count: u64, limit: u32 },

    #[error("{body:?} expects {expected} skinning matrices, got {actual}")]
    PaletteMismatch {
        body: BodyId,
        expected: usize,
        actual: usize,
    },

    #[error("{0:?} has a non-finite skinning matrix")]
    NonFinitePalette(BodyId),

    #[error("{body:?} expects {expected} morph weights, got {actual}")]
    MorphWeightMismatch {
        body: BodyId,
        expected: usize,
        actual: usize,
    },

    #[error("{0:?} has a non-finite morph weight")]
    NonFiniteMorphWeight(BodyId),

    #[error("{0:?} has a drape but no pose in these inputs")]
    UnposedDrape(BodyId),

    #[error("drape of node {} on {body:?} has {size} bytes, needs {needed}", node.index())]
    DrapeTooSmall {
        body: BodyId,
        node: NodeId,
        size: u64,
        needed: u64,
    },

    #[error("emitter {0} has a non-finite value")]
    NonFiniteEmission(u32),

    #[error("emitter {0} references an unknown region")]
    UnknownRegion(u32),

    #[error("fluid bodies changed since checkpoint")]
    CheckpointMismatch,

    #[error("time goes backwards from {from} to {to}; reset or restore a checkpoint")]
    Backwards { from: f64, to: f64 },

    #[error("substance {index} out of range for {count} substances")]
    UnknownSubstance { index: u32, count: usize },

    #[error("fluid readback returned only {0} bytes")]
    ShortReadback(usize),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
