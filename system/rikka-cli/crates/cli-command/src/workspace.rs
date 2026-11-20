use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    #[command(about = "List workspaces per output")]
    List,

    #[command(about = "Switch to a workspace")]
    Show {
        #[arg(help = "Workspace number (1-10)")]
        number: usize,
        #[arg(long, help = "Output name [default: output under the pointer]")]
        output: Option<String>,
    },
}
