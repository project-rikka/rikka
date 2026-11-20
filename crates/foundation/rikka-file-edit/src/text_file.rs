use std::path::{Path, PathBuf};

use crate::text::Text;

use crate::error::{Error, Result};
use crate::writer::{self, Precondition};

const MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct TextFile {
    path: PathBuf,
    bytes: Vec<u8>,
    text: Text,
}

impl TextFile {
    pub fn read(path: &Path) -> Result<TextFile> {
        let display = || path.display().to_string();
        let metadata = std::fs::metadata(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound(display()),
            _ => Error::Io {
                path: display(),
                reason: error.to_string(),
            },
        })?;
        if metadata.is_dir() {
            return Err(Error::Io {
                path: display(),
                reason: "it is a folder rather than a file".to_owned(),
            });
        }
        if metadata.len() > MAX_BYTES {
            return Err(Error::TooBig {
                path: display(),
                bytes: metadata.len(),
            });
        }

        let bytes = std::fs::read(path).map_err(|error| Error::Io {
            path: display(),
            reason: error.to_string(),
        })?;
        let text = Text::parse(&bytes).map_err(|_| Error::NotText(display()))?;
        Ok(TextFile {
            path: path.to_owned(),
            bytes,
            text,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn content(&self) -> &str {
        self.text.content()
    }

    pub fn write(&mut self, content: &str) -> Result<()> {
        let written = self.text.with_exact_content(content);
        let bytes = written.render();
        writer::write(&self.path, &bytes, &Precondition::Exactly(&self.bytes))?;
        self.bytes = bytes;
        self.text = written;
        Ok(())
    }
}

impl std::fmt::Debug for TextFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextFile")
            .field("path", &self.path)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
