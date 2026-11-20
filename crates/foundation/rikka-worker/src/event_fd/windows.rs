use std::os::windows::io::{
    AsHandle, AsRawHandle as _, BorrowedHandle, FromRawHandle as _, OwnedHandle,
};
use std::sync::Arc;
use std::task::Wake;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HANDLE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{
    CreateEventW, INFINITE, ResetEvent, SetEvent, WaitForMultipleObjects, WaitForSingleObject,
};

const MAX_WAIT: u32 = INFINITE - 1;

const NANOS_PER_MILLI: u128 = 1_000_000;

#[derive(Clone)]
pub struct EventNotifier {
    event: Arc<OwnedHandle>,
}

pub struct EventFd {
    event: Arc<OwnedHandle>,
}

pub fn event_fd() -> std::io::Result<(EventNotifier, EventFd)> {
    let handle = unsafe { CreateEventW(None, true, false, None) }.map_err(std::io::Error::from)?;
    let event = Arc::new(unsafe { OwnedHandle::from_raw_handle(handle.0) });
    Ok((
        EventNotifier {
            event: Arc::clone(&event),
        },
        EventFd { event },
    ))
}

fn raw(handle: BorrowedHandle<'_>) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}

impl EventNotifier {
    pub fn notify(&self) -> std::io::Result<()> {
        unsafe { SetEvent(raw(self.event.as_handle())) }.map_err(std::io::Error::from)
    }
}

impl Wake for EventNotifier {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if let Err(error) = self.notify() {
            tracing::debug!(%error, "failed to signal event");
        }
    }
}

impl AsHandle for EventFd {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.event.as_handle()
    }
}

impl EventFd {
    pub fn drain(&self) {
        if let Err(error) = unsafe { ResetEvent(raw(self.event.as_handle())) } {
            tracing::debug!(%error, "failed to reset event");
        }
    }

    pub fn wait(&self) -> std::io::Result<()> {
        self.wait_signaled([], None).map(drop)
    }

    pub fn wait_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        self.wait_signaled([], timeout).map(drop)
    }

    pub fn wait_signaled<const N: usize>(
        &self,
        handles: [BorrowedHandle<'_>; N],
        timeout: impl Into<Option<Duration>>,
    ) -> std::io::Result<[bool; N]> {
        let deadline = timeout.into().map(|timeout| Instant::now() + timeout);
        let waited: Vec<HANDLE> = handles
            .iter()
            .copied()
            .chain(std::iter::once(self.event.as_handle()))
            .map(raw)
            .collect();
        match wait_any(&waited, deadline)? {
            true => Ok(std::array::from_fn(|index| is_signaled(waited[index]))),
            false => Ok([false; N]),
        }
    }
}

fn is_signaled(handle: HANDLE) -> bool {
    let outcome = unsafe { WaitForSingleObject(handle, 0) };
    outcome == WAIT_OBJECT_0
}

fn wait_any(handles: &[HANDLE], deadline: Option<Instant>) -> std::io::Result<bool> {
    loop {
        let left = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
        let milliseconds = left.map_or(INFINITE, |left| {
            u32::try_from(left.as_nanos().div_ceil(NANOS_PER_MILLI))
                .unwrap_or(MAX_WAIT)
                .min(MAX_WAIT)
        });
        let outcome = unsafe { WaitForMultipleObjects(handles, false, milliseconds) };
        if outcome == WAIT_FAILED {
            return Err(std::io::Error::last_os_error());
        }
        if outcome != WAIT_TIMEOUT {
            return Ok(true);
        }
        if milliseconds != MAX_WAIT {
            return Ok(false);
        }
    }
}

impl std::fmt::Debug for EventNotifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventNotifier")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for EventFd {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("EventFd").finish_non_exhaustive()
    }
}
