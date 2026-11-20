use std::fmt;

use rikka_polymesh::Domain;

use crate::modifier::Purpose;
use crate::reference::MaterialRef;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkipReason {
    Hidden(Purpose),
    EditModeUnsupported,
    NotEvaluated,
    MissingObject,
    SelfReference,
    NotAMesh,
    InvalidTarget,
    MissingTexture,
    NoStrength,
    NoDirection,
    NoWidth,
    SceneEvaluated,
    InvalidSettings,
    InvalidMesh,
    TooFewFaces,
    IntersectNeedsExact,
    EmptyCollection,
    NonMeshInCollection,
    BindRequired,
    VertexCountMismatch,
}

impl fmt::Display for SkipReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkipReason::Hidden(purpose) => write!(formatter, "disabled for {purpose:?}"),
            SkipReason::EditModeUnsupported => formatter.write_str("not supported in edit mode"),
            SkipReason::NotEvaluated => formatter.write_str("evaluation not available"),
            SkipReason::MissingObject => formatter.write_str("no object assigned"),
            SkipReason::SelfReference => formatter.write_str("the object references itself"),
            SkipReason::NotAMesh => formatter.write_str("the referenced object is not a mesh"),
            SkipReason::InvalidTarget => formatter.write_str("the referenced mesh is invalid"),
            SkipReason::MissingTexture => formatter.write_str("no texture assigned"),
            SkipReason::NoStrength => formatter.write_str("zero strength"),
            SkipReason::NoDirection => formatter.write_str("no projection direction"),
            SkipReason::NoWidth => formatter.write_str("zero width"),
            SkipReason::SceneEvaluated => formatter.write_str("evaluated by the scene"),
            SkipReason::InvalidSettings => formatter.write_str("settings are out of range"),
            SkipReason::InvalidMesh => formatter.write_str("the mesh is invalid"),
            SkipReason::TooFewFaces => formatter.write_str("requires more than 3 input faces"),
            SkipReason::IntersectNeedsExact => {
                formatter.write_str("intersect is only available with the exact solver")
            }
            SkipReason::EmptyCollection => {
                formatter.write_str("non-exact solver and empty collection")
            }
            SkipReason::NonMeshInCollection => {
                formatter.write_str("the collection contains non-mesh objects")
            }
            SkipReason::BindRequired => formatter.write_str("bind data required"),
            SkipReason::VertexCountMismatch => {
                formatter.write_str("original vertex count mismatch")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransferFailure {
    TopologyMismatch(Domain),
    NoSourceEdges,
    NoSourceFaces,
    NoElements(Domain),
}

impl fmt::Display for TransferFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransferFailure::TopologyMismatch(domain) => write!(
                formatter,
                "source and destination differ in {} count, topology mapping cannot be used",
                domain.name()
            ),
            TransferFailure::NoSourceEdges => formatter.write_str("source mesh has no edges"),
            TransferFailure::NoSourceFaces => formatter.write_str("source mesh has no faces"),
            TransferFailure::NoElements(domain) => write!(
                formatter,
                "source or destination mesh has no {} elements",
                domain.name()
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteKind {
    Skipped(SkipReason),
    LevelsReduced { requested: u32, evaluated: u32 },
    CountReduced { requested: u32, evaluated: u32 },
    TransferFailed(TransferFailure),
    FaceCount(u32),
    ManifoldSolverAsExact,
    MaterialSlots(Vec<Option<MaterialRef>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub modifier: usize,
    pub kind: NoteKind,
}

impl fmt::Display for NoteKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoteKind::Skipped(reason) => write!(formatter, "skipped: {reason}"),
            NoteKind::LevelsReduced {
                requested,
                evaluated,
            } => write!(
                formatter,
                "evaluated at level {evaluated} instead of {requested} to bound its size"
            ),
            NoteKind::CountReduced {
                requested,
                evaluated,
            } => write!(
                formatter,
                "evaluated {evaluated} copies instead of {requested} to bound its size"
            ),
            NoteKind::TransferFailed(failure) => write!(formatter, "transfer failed: {failure}"),
            NoteKind::FaceCount(count) => write!(formatter, "face count: {count}"),
            NoteKind::ManifoldSolverAsExact => {
                formatter.write_str("manifold solver evaluated with the exact solver")
            }
            NoteKind::MaterialSlots(slots) => {
                write!(
                    formatter,
                    "material slots replaced by {} slots",
                    slots.len()
                )
            }
        }
    }
}
