use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rikka_compositor_types::Format;
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, memfd_create};

const ENTRY_SIZE: usize = 16;

#[derive(Debug)]
pub(super) struct FormatTable {
    fd: OwnedFd,
    size: u32,
    formats: Vec<Format>,
    indices: Vec<(Format, u16)>,
}

impl FormatTable {
    pub(super) fn new(formats: &[Format]) -> io::Result<Self> {
        let mut bytes = Vec::with_capacity(formats.len() * ENTRY_SIZE);
        let mut listed = Vec::with_capacity(formats.len());
        let mut indices: Vec<(Format, u16)> = Vec::with_capacity(formats.len());
        for format in formats {
            let at = match indices.binary_search_by_key(format, |(listed, _)| *listed) {
                Ok(_) => continue,
                Err(at) => at,
            };
            let index = u16::try_from(listed.len())
                .map_err(|_| io::Error::other("more than 65536 dmabuf formats"))?;
            indices.insert(at, (*format, index));
            listed.push(*format);
            bytes.extend_from_slice(&format.fourcc.0.to_ne_bytes());
            bytes.extend_from_slice(&[0; 4]);
            bytes.extend_from_slice(&format.modifier.0.to_ne_bytes());
        }
        let fd = memfd_create(
            "rikka-dmabuf-formats",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )?;
        let mut sent = 0;
        while sent < bytes.len() {
            sent += rustix::io::write(&fd, &bytes[sent..])?;
        }
        fcntl_add_seals(
            &fd,
            SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
        )?;
        let size =
            u32::try_from(bytes.len()).map_err(|_| io::Error::other("format table too large"))?;
        Ok(Self {
            fd,
            size,
            formats: listed,
            indices,
        })
    }

    pub(super) fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub(super) fn size(&self) -> u32 {
        self.size
    }

    pub(super) fn index(&self, format: &Format) -> Option<u16> {
        self.indices
            .binary_search_by_key(format, |(listed, _)| *listed)
            .ok()
            .map(|at| self.indices[at].1)
    }

    pub(super) fn contains(&self, format: &Format) -> bool {
        self.index(format).is_some()
    }

    pub(super) fn formats(&self) -> impl Iterator<Item = &Format> {
        self.formats.iter()
    }

    pub(super) fn sorted(&self) -> impl Iterator<Item = Format> + '_ {
        self.indices.iter().map(|(format, _)| *format)
    }
}
