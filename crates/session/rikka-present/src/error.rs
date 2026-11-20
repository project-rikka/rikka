#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Wayland(#[from] rikka_wayland::Error),

    #[error(transparent)]
    Renderer(#[from] rikka_renderer::Error),

    #[error("buffer synchronization failed")]
    Sync(#[from] std::io::Error),

    #[error("GPU cannot export dmabufs")]
    NoExport,

    #[error("compositor accepts no buffer format this GPU can export")]
    NoLayout,

    #[error("buffer released before presentation")]
    Gone,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<Error> for rikka_renderer::Error {
    fn from(error: Error) -> rikka_renderer::Error {
        match error {
            Error::Renderer(error) => error,
            other => rikka_renderer::Error::Driver(Box::new(other)),
        }
    }
}
