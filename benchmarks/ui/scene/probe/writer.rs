use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::Ordering;

use super::{CAPACITY, PROBE_ENV, SHARED_SIZE, Shared, monotonic_ns};

pub struct Probe {
    shared: Option<&'static Shared>,
}

impl Probe {
    #[must_use]
    pub fn from_env() -> Probe {
        let Some(value) = std::env::var_os(PROBE_ENV) else {
            return Probe { shared: None };
        };
        unsafe { std::env::remove_var(PROBE_ENV) };
        let fd: RawFd = value
            .to_str()
            .and_then(|fd| fd.parse().ok())
            .unwrap_or_else(|| panic!("{PROBE_ENV} is not a file descriptor: {value:?}"));
        Probe::from_fd(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    #[must_use]
    pub fn from_fd(fd: OwnedFd) -> Probe {
        let mapped = unsafe {
            rustix::mm::mmap(
                std::ptr::null_mut(),
                SHARED_SIZE,
                rustix::mm::ProtFlags::READ | rustix::mm::ProtFlags::WRITE,
                rustix::mm::MapFlags::SHARED,
                &fd,
                0,
            )
        };
        match mapped {
            Ok(address) => Probe {
                shared: Some(unsafe { &*address.cast::<Shared>() }),
            },
            Err(error) => panic!("failed to map the frame probe: {error}"),
        }
    }

    pub fn frame(&self) {
        let Some(shared) = self.shared else {
            return;
        };
        let index = shared.frames.load(Ordering::Relaxed);
        shared.stamps[index as usize % CAPACITY].store(monotonic_ns(), Ordering::Relaxed);
        shared.frames.store(index + 1, Ordering::Release);
    }
}

impl std::fmt::Debug for Probe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Probe")
            .field("recording", &self.shared.is_some())
            .finish()
    }
}
