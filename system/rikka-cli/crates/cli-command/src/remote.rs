use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum RemoteCommand {
    #[command(about = "Show where the remote desktop listens, its fingerprint and its viewer")]
    Status,

    #[command(about = "End the current viewer's connection")]
    Disconnect,
}
