mod address;
#[cfg(unix)]
mod client;
#[cfg(unix)]
mod frame;
#[cfg(unix)]
mod listener;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
mod worker;

pub use address::{
    SOCKET_ENV, display_of, session_sockets, sessions_directory, socket_path, socket_path_beside,
    socket_path_for,
};
#[cfg(unix)]
pub use client::SocketClient;
#[cfg(unix)]
pub use listener::SocketListener;
#[cfg(windows)]
pub use windows::{SocketClient, SocketListener};
