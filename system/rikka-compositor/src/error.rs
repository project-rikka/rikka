use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot create event loop: {0}")]
    EventLoop(#[source] io::Error),
    #[error("cannot open Wayland socket: {0}")]
    Socket(#[source] rikka_compositor_server::Error),
    #[error("cannot create renderer: {0}")]
    Renderer(#[source] rikka_renderer::Error),
    #[error("cannot start KMS backend: {0}")]
    Kms(#[source] rikka_compositor_kms::Error),
    #[error("cannot open input devices: {0}")]
    Input(#[source] rikka_compositor_input::Error),
    #[error("cannot compile keymap: {0}")]
    Keymap(#[source] rikka_compositor_seat::Error),
    #[error("cannot open a window on the host compositor: {0}")]
    Host(#[source] rikka_wayland::Error),
    #[error("cannot present to the host compositor: {0}")]
    Present(#[source] rikka_present::Error),
    #[error("cannot start private session bus: {0}")]
    Bus(#[source] io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
