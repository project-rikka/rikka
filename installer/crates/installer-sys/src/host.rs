use std::{
    fmt,
    fs::{self, DirBuilder, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
    process::Stdio,
};

use crate::{
    command::{Command, Output},
    error::{Error, Result},
    output::{collect, feed},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Live,
    DryRun,
    Demo,
}

impl Mode {
    #[must_use]
    pub fn is_live(self) -> bool {
        matches!(self, Self::Live)
    }
}

pub trait Host: fmt::Debug + Send + Sync {
    fn execute(&self, command: &Command, on_line: &mut dyn FnMut(&str)) -> Result<Output>;

    fn capture(&self, command: &Command) -> Result<String>;

    fn exists(&self, path: &str) -> bool;

    fn read(&self, path: &str) -> Result<String>;

    fn write(&self, path: &str, contents: &str, mode: u32) -> Result<()>;

    fn mkdir(&self, path: &str, mode: u32) -> Result<()>;

    fn which(&self, tool: &str) -> bool;

    fn euid(&self) -> u32;

    fn mode(&self) -> Mode;

    fn journal(&self) -> Vec<String> {
        Vec::new()
    }

    fn run(&self, command: &Command) -> Result<Output> {
        self.run_logged(command, &mut |_| {})
    }

    fn run_logged(&self, command: &Command, on_line: &mut dyn FnMut(&str)) -> Result<Output> {
        let output = self.execute(command, on_line)?;
        if output.succeeded() {
            Ok(output)
        } else {
            Err(Error::Failed {
                command: command.to_string(),
                status: output.status,
                stderr: if output.stderr.trim().is_empty() {
                    output.stdout.clone()
                } else {
                    output.stderr.clone()
                },
            })
        }
    }
}

#[derive(Debug, Default)]
pub struct SystemHost;

impl Host for SystemHost {
    fn execute(&self, command: &Command, on_line: &mut dyn FnMut(&str)) -> Result<Output> {
        let mut child = std::process::Command::new(&command.program)
            .args(&command.args)
            .stdin(if command.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| Error::Spawn {
                program: command.program.clone(),
                source,
            })?;

        if let Some(secret) = &command.stdin {
            feed(&mut child, secret.as_str())?;
        }

        collect(child, on_line)
    }

    fn capture(&self, command: &Command) -> Result<String> {
        let output = std::process::Command::new(&command.program)
            .args(&command.args)
            .stdin(Stdio::null())
            .output()
            .map_err(|source| Error::Spawn {
                program: command.program.clone(),
                source,
            })?;

        if !output.status.success() {
            return Err(Error::Failed {
                command: command.to_string(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn exists(&self, path: &str) -> bool {
        fs::symlink_metadata(path).is_ok()
    }

    fn read(&self, path: &str) -> Result<String> {
        fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_owned(),
            source,
        })
    }

    fn write(&self, path: &str, contents: &str, mode: u32) -> Result<()> {
        let io = |source| Error::Io {
            path: path.to_owned(),
            source,
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(mode)
            .open(path)
            .map_err(io)?;
        file.write_all(contents.as_bytes()).map_err(io)?;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(io)
    }

    fn mkdir(&self, path: &str, mode: u32) -> Result<()> {
        let io = |source| Error::Io {
            path: path.to_owned(),
            source,
        };
        DirBuilder::new()
            .recursive(true)
            .mode(mode)
            .create(path)
            .map_err(io)?;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(io)
    }

    fn which(&self, tool: &str) -> bool {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path).any(|directory| {
            let candidate = directory.join(tool);
            candidate != Path::new(tool)
                && fs::metadata(&candidate)
                    .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
    }

    fn euid(&self) -> u32 {
        fs::metadata("/proc/self").map_or(u32::MAX, |meta| meta.uid())
    }

    fn mode(&self) -> Mode {
        Mode::Live
    }
}
