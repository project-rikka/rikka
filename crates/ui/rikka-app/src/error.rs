use rikka_renderer::Error as RendererError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("renderer error: {0}")]
    Renderer(#[from] RendererError),
    #[cfg(unix)]
    #[error("no Wayland display")]
    NoDisplay,
    #[cfg(unix)]
    #[error("Wayland connection error: {0}")]
    Display(#[from] rikka_wayland::Error),
    #[cfg(windows)]
    #[error("window system error: {0}")]
    Win32(#[from] windows::core::Error),
    #[error("failed to spawn thread: {0}")]
    Runtime(#[from] std::io::Error),
    #[error("failed to load translations: {0}")]
    I18n(#[from] rikka_i18n::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("window closed")]
pub struct SendError;
