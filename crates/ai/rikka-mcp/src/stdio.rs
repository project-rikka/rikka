use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::line;

#[must_use]
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Stdio {
    command: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    cwd: Option<PathBuf>,
}

impl Stdio {
    pub fn new(command: impl Into<String>) -> Stdio {
        Stdio {
            command: command.into(),
            ..Stdio::default()
        }
    }

    pub fn parse(line: &str) -> Result<Stdio> {
        let mut words = line::words(line).ok_or(Error::Unclosed)?.into_iter();
        Ok(Stdio::new(words.next().unwrap_or_default()).args(words))
    }

    #[must_use]
    pub fn command_line(&self) -> String {
        line::spelled(
            std::iter::once(self.command.as_str()).chain(self.args.iter().map(String::as_str)),
        )
    }
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn variable(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }

    pub fn env(mut self, env: impl IntoIterator<Item = (String, String)>) -> Self {
        self.env.extend(env);
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.args
    }

    #[must_use]
    pub fn variables(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    #[must_use]
    pub fn folder(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }
}

impl std::fmt::Debug for Stdio {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Stdio")
            .field("command", &self.command)
            .field("args", &self.args)
            .field("env", &self.env.keys().collect::<Vec<_>>())
            .field("cwd", &self.cwd)
            .finish()
    }
}
