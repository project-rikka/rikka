use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum WindowCommand {
    #[command(about = "List windows")]
    List,

    #[command(about = "Focus a window")]
    Focus {
        #[arg(help = "Window ID")]
        id: String,
    },

    #[command(about = "Close a window")]
    Close {
        #[arg(help = "Window ID")]
        id: String,
    },

    #[command(about = "Minimize a floating window")]
    Minimize {
        #[arg(help = "Window ID")]
        id: String,
    },

    #[command(about = "Move a window to a workspace")]
    Move {
        #[arg(help = "Window ID")]
        id: String,
        #[arg(help = "Workspace number (1-10)")]
        workspace: usize,
    },
}
