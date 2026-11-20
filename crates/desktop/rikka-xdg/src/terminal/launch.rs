use std::path::PathBuf;
use std::process::Command;

use super::Terminal;
use crate::exec::{Exec, FieldCodes};

const COMPATIBLE_EXEC_ARG: &str = "-e";

#[derive(Clone, Debug)]
pub struct TerminalLaunch<'a> {
    terminal: &'a Terminal,
    dir: Option<PathBuf>,
    title: Option<String>,
    app_id: Option<String>,
    hold: bool,
    command: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalCommand {
    pub argv: Vec<String>,
    pub current_dir: Option<PathBuf>,
}

impl<'a> TerminalLaunch<'a> {
    pub(super) fn new(terminal: &'a Terminal) -> TerminalLaunch<'a> {
        TerminalLaunch {
            terminal,
            dir: None,
            title: None,
            app_id: None,
            hold: false,
            command: Vec::new(),
        }
    }

    #[must_use]
    pub fn dir(mut self, dir: impl Into<PathBuf>) -> TerminalLaunch<'a> {
        self.dir = Some(dir.into());
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> TerminalLaunch<'a> {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn app_id(mut self, app_id: impl Into<String>) -> TerminalLaunch<'a> {
        self.app_id = Some(app_id.into());
        self
    }

    #[must_use]
    pub fn hold(mut self) -> TerminalLaunch<'a> {
        self.hold = true;
        self
    }

    #[must_use]
    pub fn run(
        mut self,
        command: impl IntoIterator<Item = impl Into<String>>,
    ) -> TerminalLaunch<'a> {
        self.command = command.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn resolve(&self) -> Option<TerminalCommand> {
        let entry = &self.terminal.entry;
        let mut argv = Exec::expand(
            &self.terminal.exec,
            &FieldCodes {
                paths: &[],
                uris: &[],
                name: &entry.name,
                icon: entry.icon.as_deref(),
                location: entry.path.to_str(),
            },
        );
        if argv.is_empty() {
            return None;
        }
        let args = &entry.terminal_args;
        let mut current_dir = None;
        if let Some(dir) = &self.dir {
            match &args.dir {
                Some(flag) => push_valued(&mut argv, flag, &dir.to_string_lossy()),
                None => current_dir = Some(dir.clone()),
            }
        }
        if let (Some(title), Some(flag)) = (&self.title, &args.title) {
            push_valued(&mut argv, flag, title);
        }
        if let (Some(app_id), Some(flag)) = (&self.app_id, &args.app_id) {
            push_valued(&mut argv, flag, app_id);
        }
        if let (true, Some(flag)) = (self.hold, &args.hold) {
            argv.push(flag.clone());
        }
        if !self.command.is_empty() {
            match args.exec.as_deref() {
                None => argv.push(COMPATIBLE_EXEC_ARG.to_owned()),
                Some("") => {}
                Some(flag) => argv.push(flag.to_owned()),
            }
            argv.extend(self.command.iter().cloned());
        }
        Some(TerminalCommand { argv, current_dir })
    }

    #[must_use]
    pub fn command(&self) -> Option<Command> {
        let resolved = self.resolve()?;
        let (program, arguments) = resolved.argv.split_first()?;
        let mut command = Command::new(program);
        command.args(arguments);
        if let Some(dir) = &resolved.current_dir {
            command.current_dir(dir);
        }
        Some(command)
    }
}

fn push_valued(argv: &mut Vec<String>, flag: &str, value: &str) {
    match flag.ends_with('=') {
        true => argv.push(format!("{flag}{value}")),
        false => {
            argv.push(flag.to_owned());
            argv.push(value.to_owned());
        }
    }
}
