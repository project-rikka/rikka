use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use windows::Win32::Foundation::{HANDLE, WAIT_FAILED};
use windows::Win32::System::Ole::{DROPEFFECT_NONE, DoDragDrop};
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
    PeekMessageW, QS_ALLINPUT, TranslateMessage, WM_QUIT,
};

use crate::error::{Error, Result};
use crate::win32::active;
use crate::win32::runner::Runner;

const LONGEST_WAIT: u32 = INFINITE - 1;
const NANOS_PER_MILLI: u128 = 1_000_000;

pub(crate) fn run(runner: Runner) -> Result<()> {
    let runner = Rc::new(RefCell::new(runner));
    let installed = active::install(&runner);
    let started = runner.borrow_mut().start();
    if let Err(error) = started {
        runner.borrow_mut().fail(error);
    }
    loop {
        let deadline = {
            let mut turning = runner.borrow_mut();
            if turning.exiting() {
                break;
            }
            turning.turn()
        };
        run_modals(&runner);
        if runner.borrow().exiting() {
            break;
        }
        let handles = runner.borrow().wait_handles();
        if !runner.borrow().has_heard()
            && let Err(error) = wait(&handles, deadline)
        {
            runner.borrow_mut().fail(error);
            break;
        }
        runner.borrow().woke();
        if !pump() {
            break;
        }
    }
    drop(installed);
    match Rc::try_unwrap(runner) {
        Ok(runner) => runner.into_inner().finish(),
        Err(_) => Err(Error::Runtime(std::io::Error::other(
            "window runner still borrowed at exit",
        ))),
    }
}

fn run_modals(runner: &Rc<RefCell<Runner>>) {
    loop {
        let call = runner.borrow_mut().take_modal();
        let Some(call) = call else {
            break;
        };
        call.run();
    }
    let dragging = runner.borrow_mut().take_drag_loop();
    if let Some(drag) = dragging {
        let mut effect = DROPEFFECT_NONE;
        let result = unsafe { DoDragDrop(&drag.data, &drag.source, drag.allowed, &mut effect) };
        runner.borrow_mut().drag_finished(result, effect);
    }
}

fn wait(handles: &[HANDLE], deadline: Option<Instant>) -> Result<()> {
    let milliseconds = deadline.map_or(INFINITE, |deadline| {
        let left = deadline.saturating_duration_since(Instant::now());
        u32::try_from(left.as_nanos().div_ceil(NANOS_PER_MILLI))
            .unwrap_or(LONGEST_WAIT)
            .min(LONGEST_WAIT)
    });
    let outcome = unsafe {
        MsgWaitForMultipleObjectsEx(
            Some(handles),
            milliseconds,
            QS_ALLINPUT,
            MWMO_INPUTAVAILABLE,
        )
    };
    if outcome == WAIT_FAILED {
        return Err(windows::core::Error::from_thread().into());
    }
    Ok(())
}

fn pump() -> bool {
    let mut message = MSG::default();
    while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
        if message.message == WM_QUIT {
            return false;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    true
}
