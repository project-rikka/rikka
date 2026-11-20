#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("rikka rigid world: {0}")]
    Rigid(#[from] rikka_physics_rigid::Error),
    #[error("rikka geometry: {0}")]
    Geometry(#[from] rikka_physics_geometry::Error),
    #[error("rikka worker pool: {0}")]
    Workers(#[from] rikka_physics_core::Error),
    #[error("{engine} rejected the {what}")]
    Rejected {
        engine: &'static str,
        what: &'static str,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
