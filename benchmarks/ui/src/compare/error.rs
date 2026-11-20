use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    Missing(&'static str),
    Spawn {
        program: PathBuf,
        source: std::io::Error,
    },
    Compositor(String),
    Window(String),
    Input(String),
    Probe(std::io::Error),
    Process {
        pid: u32,
        source: std::io::Error,
    },
    Exited {
        program: PathBuf,
        log: PathBuf,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Missing(variable) => write!(formatter, "{variable} is not set"),
            Error::Spawn { program, source } => {
                write!(formatter, "failed to start {}: {source}", program.display())
            }
            Error::Compositor(reason) => write!(formatter, "headless compositor: {reason}"),
            Error::Window(reason) => write!(formatter, "window: {reason}"),
            Error::Input(reason) => write!(formatter, "virtual input: {reason}"),
            Error::Probe(source) => write!(formatter, "frame probe: {source}"),
            Error::Process { pid, source } => {
                write!(formatter, "failed to sample process {pid}: {source}")
            }
            Error::Exited { program, log } => write!(
                formatter,
                "{} exited early, see {}",
                program.display(),
                log.display()
            ),
        }
    }
}

impl std::error::Error for Error {}
