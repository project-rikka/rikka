use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStderr, ChildStdout, Command, Stdio};

use crate::error::{Error, Result};
use crate::failure::Failure;
use crate::progress::{Meter, Progress, Reading, Stage};

pub(crate) const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

const LITERAL: &str = ":(literal)";

pub(crate) struct Git {
    root: PathBuf,
    args: Vec<OsString>,
    paths: Vec<PathBuf>,
    differs: bool,
}

pub(crate) fn git(root: impl Into<PathBuf>) -> Git {
    Git {
        root: root.into(),
        args: Vec::new(),
        paths: Vec::new(),
        differs: false,
    }
}

impl Git {
    #[must_use]
    pub(crate) fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    #[must_use]
    pub(crate) fn args<A: Into<OsString>>(mut self, args: impl IntoIterator<Item = A>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub(crate) fn paths(mut self, paths: impl IntoIterator<Item = PathBuf>) -> Self {
        self.paths.extend(paths);
        self
    }

    #[must_use]
    pub(crate) fn differs(mut self, differs: bool) -> Self {
        self.differs = differs;
        self
    }

    pub(crate) fn read(self) -> Result<String> {
        let root = self.root.clone();
        let feeding = !self.paths.is_empty();
        let pathspecs_bytes = feeding.then(|| pathspecs(&self.paths));
        let differs = self.differs;

        let mut child = self
            .command()
            .stdin(match feeding {
                true => Stdio::piped(),
                false => Stdio::null(),
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| unavailable(&error, &root))?;

        if let Some(pathspecs_bytes) = pathspecs_bytes {
            let mut stdin = child.stdin.take().expect("stdin is piped");
            if let Err(error) = stdin.write_all(&pathspecs_bytes) {
                tracing::debug!(%error, "failed to write pathspecs to git");
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|error| Error::Unavailable(error.to_string()))?;
        let code = output.status.code();
        if output.status.success() || (differs && code == Some(1)) {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        Err(Error::Refused(error_message(
            &output.stderr,
            &output.stdout,
            code,
        )))
    }

    pub(crate) fn run(self) -> Result<()> {
        self.read().map(|_| ())
    }

    pub(crate) fn transfer(
        self,
        stages: &[Stage],
        mut report: impl FnMut(Progress),
    ) -> Result<(), Failure> {
        let root = self.root.clone();
        let mut child = self
            .command()
            .env("LC_ALL", "C")
            .env("GIT_MERGE_AUTOEDIT", "no")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| Failure::Unavailable(unavailable(&error, &root)))?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");

        let (printed, said) = std::thread::scope(|scope| {
            let printing = std::thread::Builder::new()
                .name("rikka-git-output".to_owned())
                .spawn_scoped(scope, move || printed(stdout));
            let said = said(stderr, &mut Meter::new(stages), &mut report);
            let printed = match printing {
                Ok(printing) => printing.join().unwrap_or_default(),
                Err(error) => {
                    tracing::debug!(%error, "failed to read git output");
                    String::new()
                }
            };
            (printed, said)
        });

        let status = child
            .wait()
            .map_err(|error| Failure::Unavailable(Error::Unavailable(error.to_string())))?;
        match status.success() {
            true => Ok(()),
            false => Err(Failure::Refused {
                said,
                printed,
                code: status.code(),
            }),
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new("git");
        command
            .current_dir(&self.root)
            .args(["-c", "core.quotepath=false", "-c", "color.ui=false"])
            .args(&self.args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_PAGER", "cat")
            .env("PAGER", "cat")
            .env("TERM", "dumb");
        command
    }
}

fn printed(mut stdout: ChildStdout) -> String {
    let mut bytes = Vec::new();
    if let Err(error) = stdout.read_to_end(&mut bytes) {
        tracing::debug!(%error, "failed to read git output");
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn said(
    mut stderr: ChildStderr,
    meter: &mut Meter<'_>,
    report: &mut impl FnMut(Progress),
) -> Vec<String> {
    let mut said = Vec::new();
    let mut pending = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = match stderr.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                tracing::debug!(%error, "failed to read git progress");
                break;
            }
        };
        pending.extend_from_slice(&chunk[..read]);
        while let Some(end) = pending
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
        {
            let line: Vec<u8> = pending.drain(..=end).collect();
            heard(&line[..end], meter, report, &mut said);
        }
    }
    heard(&pending, meter, report, &mut said);
    said
}

fn heard(
    line: &[u8],
    meter: &mut Meter<'_>,
    report: &mut impl FnMut(Progress),
    said: &mut Vec<String>,
) {
    let line = String::from_utf8_lossy(line);
    let line = line.trim_end();
    if line.is_empty() {
        return;
    }
    match meter.read(line) {
        Reading::Moved(progress) => report(progress),
        Reading::Same => {}
        Reading::Other => said.push(line.to_owned()),
    }
}

fn unavailable(error: &std::io::Error, root: &Path) -> Error {
    Error::Unavailable(format!("{} in {}", error, root.display()))
}

pub(crate) fn error_message(stderr: &[u8], stdout: &[u8], code: Option<i32>) -> String {
    let output = [stderr, stdout]
        .into_iter()
        .map(|stream| String::from_utf8_lossy(stream).trim().to_owned())
        .find(|text| !text.is_empty());
    output.unwrap_or_else(|| match code {
        Some(code) => format!("git exited with status {code}"),
        None => "git was stopped".to_owned(),
    })
}

fn pathspecs(paths: &[PathBuf]) -> Vec<u8> {
    let mut pathspecs_bytes = Vec::new();
    for path in paths {
        pathspecs_bytes.extend_from_slice(LITERAL.as_bytes());
        pathspecs_bytes.extend_from_slice(path.to_string_lossy().as_bytes());
        pathspecs_bytes.push(0);
    }
    pathspecs_bytes
}

pub(crate) fn fields(output: &str) -> impl Iterator<Item = &str> {
    output.split('\0').filter(|field| !field.is_empty())
}
