use crate::{BodyId, ContactId, JointId, ShapeId};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("body {0:?} does not exist")]
    UnknownBody(BodyId),
    #[error("shape {0:?} does not exist")]
    UnknownShape(ShapeId),
    #[error("joint {0:?} does not exist")]
    UnknownJoint(JointId),
    #[error("contact {0:?} is not touching")]
    UnknownContact(ContactId),
    #[error("{what} must be finite, got {value}")]
    NotFinite { what: &'static str, value: f32 },
    #[error("{what} is out of range: {value}")]
    OutOfRange { what: &'static str, value: f32 },
    #[error("{what} is not a valid spring")]
    InvalidSpring { what: &'static str },
    #[error("joint needs two distinct bodies")]
    SelfJoint,
    #[error("joint needs at least one dynamic body")]
    NoDynamicBody,
    #[error("joint parameters are invalid")]
    InvalidJoint,
    #[error("joint {0:?} has a different kind")]
    JointKind(JointId),
    #[error("material is invalid")]
    InvalidMaterial,
    #[error("body {0:?} is not kinematic")]
    NotKinematic(BodyId),
    #[error("substep count must be at least 1")]
    NoSubsteps,
    #[error("malformed encoded definition at {what}")]
    Decode { what: &'static str },
    #[error("malformed encoded definition: {0}")]
    Format(#[from] rikka_file_format::DecodeError),
    #[error(transparent)]
    Geometry(#[from] rikka_physics_geometry::Error),
    #[error(transparent)]
    Core(#[from] rikka_physics_core::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
