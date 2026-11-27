use rikka_model_core::NodeId;

use crate::figure::FigureId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cloth GPU operation failed: {0}")]
    Gpu(#[from] rikka_gpu::Error),

    #[error("{0} must be finite")]
    NonFinite(&'static str),

    #[error("step rate {0} Hz out of range {min}..={max}", min = crate::builder::MIN_RATE, max = crate::builder::MAX_RATE)]
    RateOutOfRange(f32),

    #[error("invalid cloth asset: {0}")]
    InvalidAsset(#[from] ClothAssetError),

    #[error("style contains NaN")]
    InvalidStyle,

    #[error("cloth asset has {pieces} pieces but got {styles} styles")]
    StyleCount { pieces: usize, styles: usize },

    #[error("node {} has no mesh to drape", .0.index())]
    MissingMesh(NodeId),

    #[error("node {} has no vertex {vertex}", node.index())]
    MissingVertex { node: NodeId, vertex: u32 },

    #[error("figure needs {groups} {kernel} workgroups, device limit is {limit}")]
    DeviceLimit {
        kernel: &'static str,
        groups: u64,
        limit: u32,
    },

    #[error("no cloth figure {0:?}")]
    UnknownFigure(FigureId),

    #[error("{figure:?} has no piece {piece}")]
    UnknownPiece { figure: FigureId, piece: usize },

    #[error("{figure:?} expects {expected} node matrices, got {actual}")]
    PoseMismatch {
        figure: FigureId,
        expected: usize,
        actual: usize,
    },

    #[error("{0:?} has a non-finite pose matrix")]
    NonFinitePose(FigureId),

    #[error("{figure:?} has no draped node {} for reactions", node.index())]
    UnknownReactionNode { figure: FigureId, node: NodeId },

    #[error("reaction buffer of node {} is smaller than {vertices} vertices", node.index())]
    ReactionBufferTooSmall { node: NodeId, vertices: u32 },

    #[error("cloth supports at most {max} figures, got {count}", max = crate::step::MAX_FIGURES)]
    TooManyFigures { count: usize },

    #[error("{figure:?} has {count} grabs, more than {max}", max = rikka_physics_tailor::MAX_GRABS)]
    TooManyGrabs { figure: FigureId, count: usize },

    #[error("{0:?} has a grab with non-finite point or strength")]
    NonFiniteGrab(FigureId),

    #[error("grab of {figure:?} references particles outside piece {piece}")]
    GrabOutsidePiece { figure: FigureId, piece: usize },

    #[error("checkpoint belongs to different figures")]
    ForeignCheckpoint,

    #[error("cloth readback returned only {0} bytes")]
    ShortReadback(usize),
}

#[derive(Debug, thiserror::Error)]
pub enum ClothAssetError {
    #[error("no pieces")]
    NoPieces,

    #[error("rest world matrices must be finite and invertible")]
    RestWorlds,

    #[error("body vertex {0} is non-finite or has no skin weights")]
    BodyVertex(usize),

    #[error("body collider has no node")]
    ColliderNode,

    #[error("body triangle {0} indexes past the body vertices")]
    BodyTriangle(usize),

    #[error("vertex {vertex} of node {} is bound more than once", node.index())]
    DuplicateBind { node: NodeId, vertex: u32 },

    #[error("piece {index} ({name:?}): {error}")]
    Piece {
        index: usize,
        name: String,
        #[source]
        error: PieceError,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum PieceError {
    #[error("no particles")]
    Empty,

    #[error("particle {0} has a non-finite value or no mass")]
    Particle(usize),

    #[error("particle {0} has no valid skin weights")]
    SkinWeights(usize),

    #[error("particle {0} references no body triangle")]
    SurfaceAnchor(usize),

    #[error("particle {index} has invalid parent {parent}")]
    Parent { index: usize, parent: u32 },

    #[error("particle {0} coincides with its parent")]
    CoincidentParent(usize),

    #[error("particle {0} is on a parent cycle")]
    ParentCycle(usize),

    #[error("edge {0} does not join two distinct particles")]
    Edge(usize),

    #[error("bend {0} is not a hinge of four distinct particles")]
    Bend(usize),

    #[error("tether {0} does not join two particles")]
    Tether(usize),

    #[error("triangle {0} indexes past the particles")]
    Triangle(usize),

    #[error("strand node {0} indexes past the particles")]
    StrandNode(usize),

    #[error("bind {0} references invalid triangles or strand nodes")]
    BindTarget(usize),

    #[error("bind {0} has no weight or no node")]
    BindWeight(usize),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
