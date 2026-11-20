#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bevel width must be finite and not negative, got {0}")]
    InvalidWidth(f32),

    #[error("bevel percentage must lie in 0..=100, got {0}")]
    InvalidPercent(f32),

    #[error("bevel segments must lie in 1..=1000, got {0}")]
    InvalidSegments(u32),

    #[error("bevel profile shape must lie in 0..=1, got {0}")]
    InvalidProfileShape(f32),

    #[error("bevel spread must be finite and not negative, got {0}")]
    InvalidSpread(f32),

    #[error("curve profile needs at least two points, got {0}")]
    ProfileTooShort(usize),

    #[error("curve profile points must be finite")]
    ProfileNotFinite,

    #[error("unknown vertex group {0}")]
    UnknownVertexGroup(u32),

    #[error("mesh edit failed: {0}")]
    Edit(#[from] rikka_mesh_edit::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
