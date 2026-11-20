#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Fluid(#[from] rikka_physics_fluid::Error),

    #[error(transparent)]
    Cloth(#[from] rikka_physics_cloth::Error),

    #[error(transparent)]
    Compute(#[from] rikka_gpu::Error),

    #[error("failed to spawn bake thread: {0}")]
    Thread(#[source] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
