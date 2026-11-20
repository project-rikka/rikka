#[derive(Debug, thiserror::Error)]
pub enum ClothError {
    #[error("GPU compute is unavailable")]
    Compute(#[source] rikka_renderer::Error),
    #[error("cannot fit the cloth to the figure")]
    Fit(#[from] rikka_physics_tailor::Error),
    #[error("the cloth solver failed")]
    Solver(#[from] rikka_physics::Error),
    #[error("cannot start the cloth fitting worker")]
    Worker(#[source] std::io::Error),
    #[error("the cloth fitting worker stopped")]
    WorkerStopped,
}
