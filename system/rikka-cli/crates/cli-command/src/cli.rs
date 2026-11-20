use clap::{Parser, Subcommand};
use clap_complete::Shell;

use crate::desktop::DesktopCommand;
use crate::remote::RemoteCommand;
use crate::seam::SeamCommand;
use crate::settings::SettingsCommand;
use crate::wallpaper::WallpaperCommand;
use crate::window::WindowCommand;
use crate::workspace::WorkspaceCommand;

#[derive(Debug, Parser)]
#[command(
    name = "rikka",
    bin_name = "rikka",
    version,
    about = "Control the Rikka session",
    after_help = "Connects via $RIKKA_SEAM_SOCKET or $XDG_RUNTIME_DIR/rikka/$WAYLAND_DISPLAY/.",
    propagate_version = true
)]
pub struct Cli {
    #[arg(long, global = true, help = "Output JSON")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(subcommand, about = "Manage settings")]
    Settings(SettingsCommand),

    #[command(subcommand, about = "Manage windows")]
    Window(WindowCommand),

    #[command(subcommand, about = "Manage workspaces")]
    Workspace(WorkspaceCommand),

    #[command(subcommand, about = "Manage the wallpaper")]
    Wallpaper(WallpaperCommand),

    #[command(subcommand, about = "Control the desktop shell")]
    Desktop(DesktopCommand),

    #[command(subcommand, about = "Manage the remote desktop")]
    Remote(RemoteCommand),

    #[command(subcommand, about = "Call session capabilities directly")]
    Seam(SeamCommand),

    #[command(about = "Print shell completions")]
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}
