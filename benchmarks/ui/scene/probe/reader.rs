use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::FileExt;

use super::{CAPACITY, SHARED_SIZE};

#[derive(Debug)]
pub struct Recording {
    file: File,
}

#[derive(Clone, Debug, Default)]
pub struct Stamps {
    pub frames: u64,
    pub times: Vec<u64>,
}

impl Stamps {
    #[must_use]
    pub fn between(&self, from: u64, until: u64) -> Vec<u64> {
        self.times
            .iter()
            .copied()
            .filter(|time| (from..until).contains(time))
            .collect()
    }
}

impl Recording {
    pub fn new() -> io::Result<Recording> {
        let fd = rustix::fs::memfd_create("ui-bench-probe", rustix::fs::MemfdFlags::empty())?;
        rustix::fs::ftruncate(&fd, SHARED_SIZE as u64)?;
        Ok(Recording {
            file: File::from(fd),
        })
    }

    #[must_use]
    pub fn child_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub fn frames(&self) -> io::Result<u64> {
        let mut word = [0u8; 8];
        self.file.read_exact_at(&mut word, 0)?;
        Ok(u64::from_ne_bytes(word))
    }

    pub fn stamps(&self) -> io::Result<Stamps> {
        let mut bytes = vec![0u8; SHARED_SIZE];
        self.file.read_exact_at(&mut bytes, 0)?;
        let word = |index: usize| {
            let at = index * 8;
            u64::from_ne_bytes(bytes[at..at + 8].try_into().expect("eight bytes"))
        };
        let frames = word(0);
        let kept = (frames as usize).min(CAPACITY);
        let start = frames as usize - kept;
        let times = (start..frames as usize)
            .map(|index| word(1 + index % CAPACITY))
            .collect();
        Ok(Stamps { frames, times })
    }
}
