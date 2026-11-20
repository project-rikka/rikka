use std::fmt;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::rc::Rc;

use drm::control::{Device as ControlDevice, syncobj};
use drm::{Device, DriverCapability};

#[derive(Debug, thiserror::Error)]
pub enum SyncobjError {
    #[error("DRM device does not support timeline syncobjs")]
    Unsupported,
    #[error("failed to query DRM device: {0}")]
    Query(#[from] io::Error),
}

pub(crate) struct SyncobjDevice {
    fd: OwnedFd,
}

impl AsFd for SyncobjDevice {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl Device for SyncobjDevice {}

impl ControlDevice for SyncobjDevice {}

impl SyncobjDevice {
    pub(crate) fn open(fd: OwnedFd) -> Result<Rc<Self>, SyncobjError> {
        let device = Self { fd };
        if device.get_driver_capability(DriverCapability::TimelineSyncObj)? == 0 {
            return Err(SyncobjError::Unsupported);
        }
        Ok(Rc::new(device))
    }
}

impl fmt::Debug for SyncobjDevice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SyncobjDevice")
            .field("fd", &self.fd)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) struct Timeline {
    device: Rc<SyncobjDevice>,
    handle: syncobj::Handle,
}

impl Timeline {
    pub(crate) fn import(device: &Rc<SyncobjDevice>, fd: BorrowedFd<'_>) -> io::Result<Rc<Self>> {
        let handle = device.fd_to_syncobj(fd, false)?;
        Ok(Rc::new(Self {
            device: device.clone(),
            handle,
        }))
    }
}

impl Drop for Timeline {
    fn drop(&mut self) {
        if let Err(error) = self.device.destroy_syncobj(self.handle) {
            tracing::warn!("failed to destroy client timeline: {error}");
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TimelinePoint {
    pub(crate) timeline: Rc<Timeline>,
    pub(crate) point: u64,
}

impl TimelinePoint {
    pub(crate) fn same_timeline(&self, other: &TimelinePoint) -> bool {
        Rc::ptr_eq(&self.timeline, &other.timeline)
    }

    pub(crate) fn signaled(&self) -> bool {
        let mut reached = [0u64];
        self.timeline
            .device
            .syncobj_timeline_query(&[self.timeline.handle], &mut reached, false)
            .is_ok_and(|()| reached[0] >= self.point)
    }

    pub(crate) fn notify(&self, eventfd: BorrowedFd<'_>) -> io::Result<()> {
        self.timeline
            .device
            .syncobj_eventfd(self.timeline.handle, self.point, eventfd, false)
    }

    pub(crate) fn signal(&self) {
        if let Err(error) = self
            .timeline
            .device
            .syncobj_timeline_signal(&[self.timeline.handle], &[self.point])
        {
            tracing::warn!("failed to signal release point: {error}");
        }
    }
}
