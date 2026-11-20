use std::fs::OpenOptions;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::time::Instant;

use drm::control::{Device as _, syncobj};

const DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE: libc::c_ulong = ioctl_readwrite(b'd', 0xC2, 24);
const IMPORT_SYNC_FILE: u32 = 1 << 0;

pub(crate) struct RenderNode {
    fd: OwnedFd,
    scratch: syncobj::Handle,
}

struct Card<'a>(BorrowedFd<'a>);

impl AsFd for Card<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0
    }
}

impl drm::Device for Card<'_> {}

impl drm::control::Device for Card<'_> {}

#[repr(C)]
struct SyncobjHandle {
    handle: u32,
    flags: u32,
    fd: i32,
    pad: u32,
    point: u64,
}

impl RenderNode {
    pub(crate) fn open(major: u32, minor: u32) -> io::Result<RenderNode> {
        let device = libc::makedev(major, minor);
        let path = std::fs::read_dir("/dev/dri")?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .find(|path| std::fs::metadata(path).is_ok_and(|meta| meta.rdev() == device))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no node of the renderer"))?;
        let fd: OwnedFd = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC)
            .open(path)?
            .into();
        let scratch = Card(fd.as_fd()).create_syncobj(false)?;
        Ok(RenderNode { fd, scratch })
    }

    fn card(&self) -> Card<'_> {
        Card(self.fd.as_fd())
    }

    pub(crate) fn timeline(&self) -> io::Result<(syncobj::Handle, OwnedFd)> {
        let handle = self.card().create_syncobj(false)?;
        match self.card().syncobj_to_fd(handle, false) {
            Ok(fd) => Ok((handle, fd)),
            Err(error) => {
                self.forget(handle);
                Err(error)
            }
        }
    }

    pub(crate) fn forget(&self, handle: syncobj::Handle) {
        if let Err(error) = self.card().destroy_syncobj(handle) {
            tracing::debug!("failed to destroy buffer timeline: {error}");
        }
    }

    pub(crate) fn signal(&self, timeline: syncobj::Handle, point: u64) -> io::Result<()> {
        self.card().syncobj_timeline_signal(&[timeline], &[point])
    }

    pub(crate) fn signal_after(
        &self,
        timeline: syncobj::Handle,
        point: u64,
        fence: BorrowedFd<'_>,
    ) -> io::Result<()> {
        let mut args = SyncobjHandle {
            handle: self.scratch.into(),
            flags: IMPORT_SYNC_FILE,
            fd: fence.as_raw_fd(),
            pad: 0,
            point: 0,
        };
        let done = unsafe {
            libc::ioctl(
                self.fd.as_raw_fd(),
                DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE,
                &raw mut args,
            )
        };
        if done != 0 {
            return Err(io::Error::last_os_error());
        }
        self.card()
            .syncobj_timeline_transfer(self.scratch, timeline, 0, point)
    }

    pub(crate) fn is_signaled(&self, timeline: syncobj::Handle, point: u64) -> io::Result<bool> {
        let mut value = [0];
        self.card()
            .syncobj_timeline_query(&[timeline], &mut value, false)?;
        Ok(value[0] >= point)
    }

    pub(crate) fn first_ready(
        &self,
        timelines: &[syncobj::Handle],
        points: &[u64],
        deadline: Instant,
    ) -> io::Result<Option<usize>> {
        let result = self.card().syncobj_timeline_wait(
            timelines,
            points,
            monotonic_nanos(deadline),
            false,
            true,
            true,
        );
        match result {
            Ok(index) => Ok(Some(index as usize)),
            Err(error) if error.raw_os_error() == Some(libc::ETIME) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn fence_of(&self, timeline: syncobj::Handle, point: u64) -> io::Result<OwnedFd> {
        self.card()
            .syncobj_timeline_transfer(timeline, self.scratch, point, 0)?;
        self.card().syncobj_to_fd(self.scratch, true)
    }
}

impl Drop for RenderNode {
    fn drop(&mut self) {
        self.forget(self.scratch);
    }
}

impl std::fmt::Debug for RenderNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderNode").finish_non_exhaustive()
    }
}

fn monotonic_nanos(deadline: Instant) -> i64 {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &raw mut now) };
    let left = deadline.saturating_duration_since(Instant::now());
    let nanos =
        i128::from(now.tv_sec) * 1_000_000_000 + i128::from(now.tv_nsec) + left.as_nanos() as i128;
    i64::try_from(nanos).unwrap_or(i64::MAX)
}

pub(super) const fn ioctl_readwrite(kind: u8, number: u8, size: usize) -> libc::c_ulong {
    ((3 << 30) | ((size as u32) << 16) | ((kind as u32) << 8) | number as u32) as libc::c_ulong
}

pub(super) const fn ioctl_write(kind: u8, number: u8, size: usize) -> libc::c_ulong {
    ((1 << 30) | ((size as u32) << 16) | ((kind as u32) << 8) | number as u32) as libc::c_ulong
}
