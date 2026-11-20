use crate::extent::Axis;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid surface configuration: {0}")]
    Surface(#[from] SurfaceError),
    #[error("failed to register capability: {0}")]
    Registration(rikka_seam::SeamError),
}

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("{surface} has zero {axis}; set a size or let it stretch")]
    Empty { surface: String, axis: Axis },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
