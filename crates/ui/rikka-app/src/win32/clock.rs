use std::os::windows::io::{AsHandle as _, AsRawHandle as _};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use rikka_worker::{EventFd, EventNotifier};
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::{INFINITE, WaitForSingleObject};
use windows::core::{s, w};

use crate::error::Result;

type WaitForCompositorClock = unsafe extern "system" fn(u32, *const HANDLE, u32) -> u32;

#[derive(Debug)]
pub(crate) struct Clock {
    arm: EventNotifier,
    tick: EventFd,
    armed: bool,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Clock {
    pub(crate) fn start() -> Result<Clock> {
        let (arm, armed) = rikka_worker::event_fd()?;
        let (ticks, tick) = rikka_worker::event_fd()?;
        let stopping = Arc::new(AtomicBool::new(false));
        let stops = Arc::clone(&stopping);
        let thread = std::thread::Builder::new()
            .name("rikka-app-clock".to_owned())
            .spawn(move || run(&armed, &ticks, &stops))?;
        Ok(Clock {
            arm,
            tick,
            armed: false,
            stopping,
            thread: Some(thread),
        })
    }

    pub(crate) fn arm(&mut self) {
        if self.armed {
            return;
        }
        match self.arm.notify() {
            Ok(()) => self.armed = true,
            Err(error) => tracing::warn!("failed to wait for the display: {error}"),
        }
    }

    pub(crate) fn handle(&self) -> HANDLE {
        HANDLE(self.tick.as_handle().as_raw_handle())
    }

    pub(crate) fn take_tick(&mut self) -> bool {
        if unsafe { WaitForSingleObject(self.handle(), 0) } != WAIT_OBJECT_0 {
            return false;
        }
        self.tick.drain();
        self.armed = false;
        true
    }
}

impl Drop for Clock {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Err(error) = self.arm.notify() {
            tracing::warn!("failed to stop the display clock: {error}");
            return;
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("display clock panicked");
        }
    }
}

fn run(armed: &EventFd, ticks: &EventNotifier, stopping: &AtomicBool) {
    let compositor = compositor_clock();
    loop {
        if let Err(error) = armed.wait() {
            tracing::error!("display clock failed: {error}");
            return;
        }
        armed.drain();
        if stopping.load(Ordering::Acquire) {
            return;
        }
        match compositor {
            Some(wait) => {
                unsafe { wait(0, std::ptr::null(), INFINITE) };
            }
            None => {
                if let Err(error) = unsafe { DwmFlush() } {
                    tracing::debug!("composition is not running: {error}");
                }
            }
        }
        if let Err(error) = ticks.notify() {
            tracing::error!("failed to report a display tick: {error}");
            return;
        }
    }
}

fn compositor_clock() -> Option<WaitForCompositorClock> {
    let library = unsafe { LoadLibraryW(w!("dcomp.dll")) }
        .inspect_err(|error| tracing::debug!("directcomposition unavailable: {error}"))
        .ok()?;
    let address = unsafe { GetProcAddress(library, s!("DCompositionWaitForCompositorClock")) }?;
    Some(unsafe {
        std::mem::transmute::<unsafe extern "system" fn() -> isize, WaitForCompositorClock>(address)
    })
}
