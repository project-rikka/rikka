use crate::ids::{EdgeId, Element, FaceId, LoopId, VertId};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("dead vertex {0:?}")]
    DeadVert(VertId),

    #[error("dead edge {0:?}")]
    DeadEdge(EdgeId),

    #[error("dead loop {0:?}")]
    DeadLoop(LoopId),

    #[error("dead face {0:?}")]
    DeadFace(FaceId),

    #[error("edge needs two distinct vertices")]
    DegenerateEdge,

    #[error("face needs at least three corners")]
    TooFewCorners,

    #[error("face repeats an edge between its corners")]
    RepeatedEdge,

    #[error("no edge joins {0:?} and {1:?}")]
    MissingEdge(VertId, VertId),

    #[error("face already exists as {0:?}")]
    DuplicateFace(FaceId),

    #[error("edges do not form one closed boundary")]
    OpenBoundary,

    #[error("loops are adjacent")]
    AdjacentLoops,

    #[error("loops belong to different faces")]
    LoopsInDifferentFaces,

    #[error("vertex {0:?} is not in edge {1:?}")]
    VertNotInEdge(VertId, EdgeId),

    #[error("vertex {0:?} is not in face {1:?}")]
    VertNotInFace(VertId, FaceId),

    #[error("edge {0:?} is not in face {1:?}")]
    EdgeNotInFace(EdgeId, FaceId),

    #[error("vertex {0:?} does not have exactly two edges")]
    NotTwoEdges(VertId),

    #[error("operation would leave a doubled edge")]
    DoubleEdge,

    #[error("operation would leave a doubled face")]
    DoubleFace,

    #[error("vertex {0:?} is not manifold")]
    NonManifoldVert(VertId),

    #[error("edge {0:?} is not manifold")]
    NonManifoldEdge(EdgeId),

    #[error("faces do not form one joinable region")]
    Unjoinable,

    #[error("edge {0:?} cannot rotate")]
    Unrotatable(EdgeId),

    #[error("vertex {0:?} cannot dissolve")]
    Undissolvable(VertId),

    #[error("splicing joins an element with itself")]
    SelfSplice,

    #[error("edges do not share both vertices")]
    EdgesNotCoincident,

    #[error("no face holds both vertices as non-adjacent corners")]
    NoSharedFace,

    #[error("element {0:?} is not alive")]
    DeadElement(Element),

    #[error("attribute {0} already exists")]
    AttributeExists(String),

    #[error("attribute {0} is reserved")]
    ReservedAttribute(String),

    #[error("unknown attribute {0}")]
    UnknownAttribute(String),

    #[error("primitive parameter {0} is out of range")]
    PrimitiveParameter(&'static str),

    #[error("invalid polygon mesh: {0}")]
    Polymesh(#[from] rikka_polymesh::Error),

    #[error("corrupt edit mesh: {0}")]
    Corrupt(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
