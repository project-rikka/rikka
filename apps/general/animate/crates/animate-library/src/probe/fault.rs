use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Fault {
    #[error("{0}")]
    Io(#[from] io::Error),

    #[error("not a {0} file")]
    Foreign(&'static str),

    #[error("truncated file")]
    Truncated,

    #[error("malformed {0}")]
    Malformed(&'static str),

    #[error("file too large")]
    TooLarge,

    #[error("cancelled")]
    Cancelled,

    #[error("{0}")]
    Import(String),
}

impl Fault {
    pub(crate) fn read(error: io::Error) -> Fault {
        match error.kind() {
            io::ErrorKind::UnexpectedEof => Fault::Truncated,
            _ => Fault::Io(error),
        }
    }
}

pub(crate) type Probed<T> = Result<T, Fault>;

pub(crate) fn read_limited(path: &Path, limit: u64) -> Probed<Vec<u8>> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    if size > limit {
        return Err(Fault::TooLarge);
    }
    let mut bytes = Vec::with_capacity(usize::try_from(size).map_err(|_| Fault::TooLarge)?);
    file.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub(crate) fn text(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    clean(&text)
}

pub(crate) fn clean(text: &str) -> Option<String> {
    let text = text.replace("\r\n", "\n");
    let text = text.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    (!text.is_empty()).then(|| text.to_owned())
}
