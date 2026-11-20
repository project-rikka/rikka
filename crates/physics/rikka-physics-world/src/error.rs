use crate::{BodyId, CharacterId};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tick rate must be at least 1 Hz")]
    NoTickRate,
    #[error("substep count must be at least 1")]
    NoSubsteps,
    #[error("{what} must be finite and positive, got {value}")]
    NotPositive { what: &'static str, value: f64 },
    #[error("{what} must be finite")]
    NotFinite { what: &'static str },
    #[error("character {0:?} does not exist")]
    UnknownCharacter(CharacterId),
    #[error("body {0:?} is not kinematic")]
    NotKinematic(BodyId),
    #[error("malformed recording: {0}")]
    MalformedRecording(&'static str),
    #[error("unreadable recording: {0}")]
    Recording(#[from] rikka_file_format::DecodeError),
    #[error("recording must start before the first tick")]
    RecordingAfterStart,
    #[error("failed to spawn stepping thread")]
    Thread(#[source] std::io::Error),
    #[error("stepping thread stopped")]
    SteppingStopped,
    #[error("rigid-body request failed")]
    Rigid(#[from] rikka_physics_rigid::Error),
    #[error("failed to start worker pool")]
    Workers(#[from] rikka_physics_core::Error),
    #[error("CPU lacks required SIMD instructions")]
    Simd(#[from] rikka_physics_math::SimdUnavailable),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
