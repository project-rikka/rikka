use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub const LINE_BYTES: usize = 16 * 1024;

pub const TRANSCRIPT_BYTES: u64 = 64 * 1024 * 1024;

const FILE_MODE: u32 = 0o600;

const ELLIPSIS: char = '…';

const CUT: &str = "rikka-nixctl: the rest of this output is not kept";

#[derive(Debug)]
pub struct Transcript {
    path: PathBuf,
    file: File,
    length: u64,
    full: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub offset: u64,
    pub text: String,
}

impl Transcript {
    pub(crate) fn create(path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .mode(FILE_MODE)
            .open(&path)
            .map_err(|source| Error::Write {
                path: path.clone(),
                source,
            })?;
        Ok(Self {
            path,
            file,
            length: 0,
            full: false,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&mut self, line: &str) -> Result<Option<Written>> {
        if self.full {
            return Ok(None);
        }
        let text = capped(line);
        let text =
            match self.length + text.len() as u64 + 1 + CUT.len() as u64 + 1 > TRANSCRIPT_BYTES {
                true => {
                    self.full = true;
                    CUT.to_owned()
                }
                false => text,
            };
        let mut record = String::with_capacity(text.len() + 1);
        record.push_str(&text);
        record.push('\n');
        self.file
            .write_all(record.as_bytes())
            .map_err(|source| Error::Write {
                path: self.path.clone(),
                source,
            })?;
        let offset = self.length;
        self.length += record.len() as u64;
        Ok(Some(Written { offset, text }))
    }
}

fn capped(line: &str) -> String {
    let single = line.replace(['\n', '\r'], " ");
    if single.len() <= LINE_BYTES {
        return single;
    }
    let room = LINE_BYTES - ELLIPSIS.len_utf8();
    let cut = (0..=room)
        .rev()
        .find(|at| single.is_char_boundary(*at))
        .unwrap_or(0);
    let mut kept = single[..cut].to_owned();
    kept.push(ELLIPSIS);
    kept
}
