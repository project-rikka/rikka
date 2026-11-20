use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct DrmFd(Arc<OwnedFd>);

impl DrmFd {
    pub(crate) fn new(fd: OwnedFd) -> Self {
        Self(Arc::new(fd))
    }

    pub(crate) fn into_owned(self) -> Result<OwnedFd, Self> {
        Arc::try_unwrap(self.0).map_err(Self)
    }
}

impl AsFd for DrmFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for DrmFd {}

impl drm::control::Device for DrmFd {}
