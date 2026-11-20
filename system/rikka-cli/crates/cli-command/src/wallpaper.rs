use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum WallpaperCommand {
    #[command(about = "List wallpapers")]
    List,

    #[command(about = "Print the current wallpaper")]
    Current,

    #[command(about = "Set the wallpaper")]
    Set {
        #[arg(help = "Wallpaper ID")]
        id: String,
    },

    #[command(about = "Remove the wallpaper")]
    Clear,

    #[command(about = "Rescan the wallpaper folder")]
    Refresh,
}
