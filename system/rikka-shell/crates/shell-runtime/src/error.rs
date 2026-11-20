use rikka_renderer::Error as RendererError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Shell(#[from] shell_kit::Error),
    #[error("renderer error: {0}")]
    Renderer(#[from] RendererError),
    #[error("wayland error: {0}")]
    Wayland(#[from] rikka_wayland::Error),
    #[error("failed to create event loop waker: {0}")]
    LoopWaker(#[from] std::io::Error),
    #[error("failed to create wall clock timer: {0}")]
    WallClock(std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
