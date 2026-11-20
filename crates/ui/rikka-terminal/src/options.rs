use std::path::PathBuf;

use alacritty_terminal::tty;

use crate::clipboard::ClipboardAccess;

pub(crate) const SCROLLBACK: usize = 10_000;

const TERM: &str = "xterm-256color";

#[derive(Clone, Debug)]
#[must_use]
pub struct Options {
    shell: Option<(String, Vec<String>)>,
    env: Vec<(String, String)>,
    scrollback: usize,
    working_directory: Option<PathBuf>,
    access: ClipboardAccess,
}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

impl Options {
    pub fn new() -> Self {
        Self {
            shell: None,
            env: vec![
                (String::from("TERM"), String::from(TERM)),
                (String::from("COLORTERM"), String::from("truecolor")),
            ],
            scrollback: SCROLLBACK,
            working_directory: None,
            access: ClipboardAccess::new(),
        }
    }

    pub fn shell(
        mut self,
        program: impl Into<String>,
        args: impl IntoIterator<Item = String>,
    ) -> Self {
        self.shell = Some((program.into(), args.into_iter().collect()));
        self
    }

    pub fn working_directory(mut self, at: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(at.into());
        self
    }

    pub fn scrollback(mut self, lines: usize) -> Self {
        self.scrollback = lines;
        self
    }

    pub fn clipboard(mut self, access: ClipboardAccess) -> Self {
        self.access = access;
        self
    }

    pub(crate) fn scrollback_lines(&self) -> usize {
        self.scrollback
    }

    pub(crate) fn access(&self) -> ClipboardAccess {
        self.access
    }

    pub(crate) fn to_pty(&self) -> tty::Options {
        tty::Options {
            shell: self
                .shell
                .as_ref()
                .map(|(program, args)| tty::Shell::new(program.clone(), args.clone())),
            working_directory: self.working_directory.clone(),
            drain_on_exit: true,
            env: self.env.iter().cloned().collect(),
        }
    }
}
