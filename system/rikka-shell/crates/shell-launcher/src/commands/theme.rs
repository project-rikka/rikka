use rikka_appearance::Appearance;
use rikka_theme::Variant;
use rikka_ui::{MaterialSymbol, tr};
use shell_kit::appearance;

use crate::commands::chosen::Chosen;
use crate::commands::command::{Command, CommandInfo};
use crate::provider::{Activation, Hit};

const ID: &str = "theme";

const IN_USE: &str = "shell.launcher.theme.in-use";
const DARK: &str = "shell.launcher.theme.dark";
const LIGHT: &str = "shell.launcher.theme.light";

#[derive(Debug)]
pub(crate) struct ThemeCommand {
    current: Chosen,
}

impl ThemeCommand {
    #[must_use]
    pub(crate) fn new(current: Chosen) -> ThemeCommand {
        ThemeCommand { current }
    }

    fn installed(&self) -> Vec<Hit> {
        let registry = match Appearance::current().registry() {
            Ok(registry) => registry,
            Err(error) => {
                tracing::warn!("failed to read themes: {error}");
                return Vec::new();
            }
        };
        registry
            .iter()
            .map(|spec| {
                let row = Hit::new(spec.slug.as_str(), spec.name.as_str());
                if self.current.is(&spec.slug) {
                    return row.in_use().subtitle(tr!(IN_USE));
                }
                row.subtitle(tr!(match spec.variant {
                    Variant::Dark => DARK,
                    Variant::Light => LIGHT,
                }))
            })
            .collect()
    }
}

impl Command for ThemeCommand {
    fn info(&self) -> CommandInfo {
        CommandInfo::new(ID, tr!("shell.launcher.command.theme"))
            .summary(tr!("shell.launcher.theme.summary"))
            .icon(MaterialSymbol::PALETTE)
    }

    fn open(&mut self) -> Vec<Hit> {
        self.installed()
    }

    fn apply(&mut self, id: &str) -> Activation {
        Activation::ask(appearance::write(appearance::THEME, id))
    }
}
