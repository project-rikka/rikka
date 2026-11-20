mod app;
mod device;
mod error;
mod instance;
mod launch;
mod preload;
pub mod prelude;
mod sender;
mod tasks;
#[cfg(unix)]
mod wayland;
#[cfg(windows)]
mod win32;
mod window;

#[cfg(unix)]
use wayland as backend;
#[cfg(windows)]
use win32 as backend;

pub use app::App;
pub use error::{Error, Result, SendError};
pub use instance::Instance;
pub use sender::Sender;
pub use window::{Factory, IntoWindows, Opening, Window, WindowParent, WindowShape, open_window};

pub use rikka_heap::mimalloc;
pub use rikka_instance::LaunchRequest;
