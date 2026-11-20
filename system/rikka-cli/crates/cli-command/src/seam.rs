use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum SeamCommand {
    #[command(about = "List capabilities, methods and events")]
    Capabilities,

    #[command(
        about = "Call a method and print the result",
        after_help = r#"Examples:
  rikka seam call rikka.toplevels state
  rikka seam call rikka.settings get '{"domain":"rikka.input","key":"close-window"}'"#
    )]
    Call {
        #[arg(help = "Capability name")]
        capability: String,
        #[arg(help = "Method name")]
        method: String,
        #[arg(value_name = "JSON", help = "Parameters")]
        params: Option<String>,
    },

    #[command(about = "Stream events")]
    Watch {
        #[arg(help = "Only this capability")]
        capability: Option<String>,
    },
}
