use std::cell::Cell;
use std::ffi::{c_int, c_void};
use std::ptr;
use std::sync::atomic::{Ordering, compiler_fence};
use std::sync::{Once, OnceLock};

thread_local! {
    static GUARDED: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
    static FAULTED: Cell<bool> = const { Cell::new(false) };
}

static INSTALL: Once = Once::new();
static PREVIOUS: OnceLock<libc::sigaction> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BusError;

pub(crate) fn guarded<R>(
    range: (*const u8, usize),
    access: impl FnOnce() -> R,
) -> Result<R, BusError> {
    install();
    let outer = GUARDED.with(|guarded| guarded.replace(Some((range.0 as usize, range.1))));
    let outer_fault = FAULTED.with(|faulted| faulted.replace(false));
    compiler_fence(Ordering::SeqCst);
    let result = access();
    compiler_fence(Ordering::SeqCst);
    let faulted = FAULTED.with(|faulted| faulted.replace(outer_fault));
    GUARDED.with(|guarded| guarded.set(outer));
    if faulted { Err(BusError) } else { Ok(result) }
}

fn install() {
    INSTALL.call_once(|| {
        let mut previous: libc::sigaction = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigaction(libc::SIGBUS, ptr::null(), &mut previous) } != 0 {
            tracing::warn!("failed to read SIGBUS disposition, shm access is unguarded");
            return;
        }
        let _ = PREVIOUS.set(previous);
        let handler: extern "C" fn(c_int, *mut libc::siginfo_t, *mut c_void) = on_sigbus;
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = handler as libc::sighandler_t;
        action.sa_flags = libc::SA_SIGINFO | libc::SA_NODEFER;
        unsafe { libc::sigemptyset(&mut action.sa_mask) };
        if unsafe { libc::sigaction(libc::SIGBUS, &action, ptr::null_mut()) } != 0 {
            tracing::warn!("failed to install SIGBUS handler, shm access is unguarded");
        }
    });
}

extern "C" fn on_sigbus(_signal: c_int, info: *mut libc::siginfo_t, _context: *mut c_void) {
    let address = unsafe { (*info).si_addr() } as usize;
    if remap_guarded(address) {
        return;
    }
    let fallback = PREVIOUS.get().copied().unwrap_or_else(|| {
        let mut default: libc::sigaction = unsafe { std::mem::zeroed() };
        default.sa_sigaction = libc::SIG_DFL;
        default
    });
    unsafe { libc::sigaction(libc::SIGBUS, &fallback, ptr::null_mut()) };
}

fn remap_guarded(address: usize) -> bool {
    let Some((start, len)) = GUARDED.with(Cell::get) else {
        return false;
    };
    if address < start || address - start >= len {
        return false;
    }
    let mapped = unsafe {
        libc::mmap(
            start as *mut c_void,
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_FIXED | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if mapped == libc::MAP_FAILED {
        return false;
    }
    FAULTED.with(|faulted| faulted.set(true));
    true
}
