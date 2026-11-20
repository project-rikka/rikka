use std::collections::BTreeMap;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

use rustix::fs::{Mode, OFlags, open};

use crate::git;

const SNIFFED: usize = 8_000;

const LARGEST: u64 = 16 * 1024 * 1024;

const BUDGET: u64 = 64 * 1024 * 1024;

const CHUNK: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stat {
    added: usize,
    removed: usize,
}

impl Stat {
    #[must_use]
    pub fn new(added: usize, removed: usize) -> Stat {
        Stat { added, removed }
    }

    #[must_use]
    pub fn added(self) -> usize {
        self.added
    }

    #[must_use]
    pub fn removed(self) -> usize {
        self.removed
    }
}

impl std::iter::Sum for Stat {
    fn sum<I: Iterator<Item = Stat>>(stats: I) -> Stat {
        stats.fold(Stat::default(), |sum, stat| {
            Stat::new(sum.added + stat.added, sum.removed + stat.removed)
        })
    }
}

pub(crate) fn numstat(output: &str) -> BTreeMap<PathBuf, Stat> {
    let mut stats = BTreeMap::new();
    let mut fields = git::fields(output);
    while let Some(field) = fields.next() {
        let mut parts = field.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let path = match path {
            "" => match (fields.next(), fields.next()) {
                (Some(_), Some(to)) => to,
                _ => break,
            },
            path => path,
        };
        if let (Ok(added), Ok(removed)) = (added.parse(), removed.parse()) {
            stats.insert(PathBuf::from(path), Stat::new(added, removed));
        }
    }
    stats
}

pub(crate) struct Untracked {
    chunk: Vec<u8>,
    budget: u64,
}

impl Default for Untracked {
    fn default() -> Untracked {
        Untracked {
            chunk: vec![0; CHUNK],
            budget: BUDGET,
        }
    }
}

impl Untracked {
    pub(crate) fn count(&mut self, at: &Path) -> Option<Stat> {
        if at.symlink_metadata().ok()?.is_symlink() {
            return Some(Stat::new(1, 0));
        }
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let file = File::from(open(at, flags, Mode::empty()).ok()?);
        let metadata = file.metadata().ok()?;
        let size = metadata.len();
        if !metadata.is_file() || size > LARGEST || size > self.budget {
            return None;
        }
        self.budget -= size;
        counted(file.take(size), &mut self.chunk).map(|added| Stat::new(added, 0))
    }
}

fn counted(mut read: impl Read, chunk: &mut [u8]) -> Option<usize> {
    let mut sniffed = 0;
    let mut lines = 0;
    let mut last = None;
    loop {
        let filled = match read.read(chunk) {
            Ok(0) => break,
            Ok(filled) => filled,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        let filled_chunk = &chunk[..filled];
        if sniffed < SNIFFED {
            let sniffing = &filled_chunk[..filled_chunk.len().min(SNIFFED - sniffed)];
            if sniffing.contains(&0) {
                return None;
            }
            sniffed += sniffing.len();
        }
        lines += filled_chunk.iter().filter(|&&byte| byte == b'\n').count();
        last = filled_chunk.last().copied();
    }
    match last {
        Some(byte) if byte != b'\n' => Some(lines + 1),
        _ => Some(lines),
    }
}
