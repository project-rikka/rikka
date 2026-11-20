use std::fmt;

use installer_core::Secret;

#[derive(Clone, Debug, Default)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub stdin: Option<Secret>,
}

impl Command {
    #[must_use]
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn nix() -> Self {
        Self::new("nix").args(["--extra-experimental-features", "nix-command flakes"])
    }

    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn stdin(mut self, secret: Secret) -> Self {
        self.stdin = Some(secret);
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.status == 0
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.program)?;
        for arg in &self.args {
            if arg.is_empty() || arg.contains(char::is_whitespace) {
                write!(f, " '{arg}'")?;
            } else {
                write!(f, " {arg}")?;
            }
        }
        Ok(())
    }
}
