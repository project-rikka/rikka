use std::path::Path;
use std::process::Command;

use files_core::Location;
use rikka_open::{Error, Result, spawn_detached};
use rikka_xdg::{DesktopEntry, Target, Terminals};

const TERMINAL: &str = "terminal";

pub fn launch(entry: &DesktopEntry, targets: &[Location]) -> Result<()> {
    for arguments in entry.command_lines(&opened(targets)) {
        let mut arguments = arguments.into_iter();
        let Some(program) = arguments.next() else {
            continue;
        };
        spawn_detached(Command::new(program).args(arguments))?;
    }
    Ok(())
}

pub fn open_with_desktop(targets: &[Location]) -> Result<()> {
    for target in targets {
        spawn_detached(Command::new("xdg-open").arg(target.as_str()))?;
    }
    Ok(())
}

pub fn open_terminal(at: &Path) -> Result<()> {
    let not_installed = || Error::NotInstalled {
        program: TERMINAL.to_owned(),
    };
    let terminal = Terminals::session()
        .and_then(|terminals| terminals.preferred())
        .ok_or_else(not_installed)?;
    let mut command = terminal
        .launch()
        .dir(at)
        .command()
        .ok_or_else(not_installed)?;
    spawn_detached(&mut command).map(drop)
}

fn opened(targets: &[Location]) -> Vec<Target> {
    targets
        .iter()
        .map(|target| Target {
            path: target.display(),
            uri: target.as_str().to_owned(),
        })
        .collect()
}
