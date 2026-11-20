use std::io::{self, Read};

use crc32fast::Hasher;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Integrity {
    Unbroken,
    ChecksumMismatch,
    ReadFailed,
}

pub(crate) struct CrcReader<R> {
    inner: R,
    hasher: Hasher,
    expected: u32,
    integrity: Integrity,
}

impl<R: Read> CrcReader<R> {
    pub(crate) fn new(inner: R, expected: u32) -> CrcReader<R> {
        CrcReader {
            inner,
            hasher: Hasher::new(),
            expected,
            integrity: Integrity::Unbroken,
        }
    }

    pub(crate) fn integrity(&self) -> Integrity {
        self.integrity
    }
}

impl<R: Read> Read for CrcReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let count = match self.inner.read(buf) {
            Ok(count) => count,
            Err(error) => {
                if error.kind() != io::ErrorKind::Interrupted {
                    self.integrity = Integrity::ReadFailed;
                }
                return Err(error);
            }
        };

        if count > 0 {
            self.hasher.update(&buf[..count]);
            return Ok(count);
        }

        if self.hasher.clone().finalize() != self.expected {
            self.integrity = Integrity::ChecksumMismatch;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the entry does not match the checksum the archive records for it",
            ));
        }
        Ok(0)
    }
}
