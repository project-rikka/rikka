mod cli;
mod completions;
mod desktop;
mod remote;
mod seam;
mod settings;
mod wallpaper;
mod window;
mod workspace;

pub use clap_complete::Shell;

pub use crate::cli::{Cli, Command};
pub use crate::completions::write_completions;
pub use crate::desktop::DesktopCommand;
pub use crate::remote::RemoteCommand;
pub use crate::seam::SeamCommand;
pub use crate::settings::SettingsCommand;
pub use crate::wallpaper::WallpaperCommand;
pub use crate::window::WindowCommand;
pub use crate::workspace::WorkspaceCommand;
