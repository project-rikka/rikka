use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum DesktopCommand {
    #[command(about = "Show the shell's status")]
    Status,

    #[command(about = "Start the shell")]
    Start,

    #[command(about = "Stop the shell, keeping windows open")]
    Stop,

    #[command(about = "Restart the shell")]
    Restart,
}
