use std::io;
use std::num::NonZeroU32;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

use drm::control::Device as ControlDevice;
use rustix::fs::{Mode, OFlags};

use super::types::LeaseGrant;

struct Card<'a>(BorrowedFd<'a>);

impl AsFd for Card<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0
    }
}

impl drm::Device for Card<'_> {}

impl ControlDevice for Card<'_> {}

pub fn create_lease(master: BorrowedFd<'_>, objects: &[u32]) -> io::Result<LeaseGrant> {
    let objects = objects
        .iter()
        .map(|object| {
            NonZeroU32::new(*object)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "object id zero"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let (lessee, fd) = Card(master).create_lease(&objects, OFlags::CLOEXEC.bits())?;
    Ok(LeaseGrant { lessee, fd })
}

pub fn revoke_lease(master: BorrowedFd<'_>, lessee: NonZeroU32) -> io::Result<()> {
    Card(master).revoke_lease(lessee)
}

pub fn open_lease_device(path: &Path) -> io::Result<OwnedFd> {
    let fd = rustix::fs::open(path, OFlags::RDWR | OFlags::CLOEXEC, Mode::empty())?;
    match drm::Device::release_master_lock(&Card(fd.as_fd())) {
        Err(error) if error.raw_os_error() != Some(rustix::io::Errno::INVAL.raw_os_error()) => {
            Err(error)
        }
        _ => Ok(fd),
    }
}
