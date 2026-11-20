use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum SettingsCommand {
    #[command(about = "List domains")]
    Domains,

    #[command(about = "List a domain's values and sources")]
    List {
        #[arg(help = "Settings domain")]
        domain: String,
    },

    #[command(about = "Show a domain's schema")]
    Describe {
        #[arg(help = "Settings domain")]
        domain: String,
    },

    #[command(
        about = "Print a value as TOML",
        after_help = "Examples:\n  rikka settings get rikka.input close-window"
    )]
    Get {
        #[arg(help = "Settings domain")]
        domain: String,
        #[arg(help = "Setting key")]
        key: String,
    },

    #[command(
        about = "Set a value",
        after_help = "Examples:\n  \
                      rikka settings set rikka.input close-window Super+W\n  \
                      rikka settings set rikka.input repeat-delay 400\n  \
                      rikka settings set rikka.appearance taskbar-indicators '[\"input-method\"]'"
    )]
    Set {
        #[arg(help = "Settings domain")]
        domain: String,
        #[arg(help = "Setting key")]
        key: String,
        #[arg(
            allow_hyphen_values = true,
            help = "TOML literal; bare words are strings"
        )]
        value: String,
    },

    #[command(about = "Reset to the system or default value")]
    Reset {
        #[arg(help = "Settings domain")]
        domain: String,
        #[arg(help = "Setting key")]
        key: String,
    },
}
