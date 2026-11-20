use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use smallvec::SmallVec;

use super::buffer::Dmabuf;

const DMA_BUF_SYNC_READ: u32 = 1;
const DMA_BUF_IOCTL_EXPORT_SYNC_FILE: libc::Ioctl = 0xc008_6202;

#[repr(C)]
struct ExportSyncFile {
    flags: u32,
    fd: i32,
}

pub(crate) fn pending_fences(dmabuf: &Dmabuf) -> SmallVec<[OwnedFd; 1]> {
    dmabuf
        .planes
        .iter()
        .enumerate()
        .filter(|(index, _)| dmabuf.fenced_planes & (1 << index) != 0)
        .filter_map(|(_, plane)| pending_fence(plane.fd.as_fd()))
        .collect()
}

fn pending_fence(dmabuf: BorrowedFd<'_>) -> Option<OwnedFd> {
    let fence = export_sync_file(dmabuf).or_else(|| dmabuf.try_clone_to_owned().ok())?;
    (!signaled(fence.as_fd())).then_some(fence)
}

fn export_sync_file(dmabuf: BorrowedFd<'_>) -> Option<OwnedFd> {
    let mut request = ExportSyncFile {
        flags: DMA_BUF_SYNC_READ,
        fd: -1,
    };
    let result = unsafe {
        libc::ioctl(
            dmabuf.as_raw_fd(),
            DMA_BUF_IOCTL_EXPORT_SYNC_FILE,
            &mut request as *mut ExportSyncFile,
        )
    };
    (result == 0 && request.fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(request.fd) })
}

fn signaled(fence: BorrowedFd<'_>) -> bool {
    let mut fds = [PollFd::new(&fence, PollFlags::IN)];
    let immediately = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    match poll(&mut fds, Some(&immediately)) {
        Ok(0) => false,
        Ok(_) => !fds[0].revents().is_empty(),
        Err(_) => true,
    }
}
