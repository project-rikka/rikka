use std::time::Duration;

use rikka_picture::RefreshRate;
use rikka_types::PointerButton;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, HTCLIENT, KillTimer, SWP_NOACTIVATE, SWP_NOZORDER, SetCursor, SetTimer,
    SetWindowPos, WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_ENTERMENULOOP, WM_ENTERSIZEMOVE, WM_ERASEBKGND, WM_EXITMENULOOP, WM_EXITSIZEMOVE,
    WM_GETMINMAXINFO, WM_IME_CHAR, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_SETCONTEXT,
    WM_IME_STARTCOMPOSITION, WM_INPUT, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCCALCSIZE,
    WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE,
    WM_NCMOUSEMOVE, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERENTER, WM_POINTERLEAVE,
    WM_POINTERUP, WM_POINTERUPDATE, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS,
    WM_SIZE, WM_SYSCHAR, WM_TIMER, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1,
};

use crate::win32::active::with_runner;
use crate::win32::heard::{self, Heard};
use crate::win32::input::cursor::Pointer;
use crate::win32::input::ime::Ime;
use crate::win32::input::{Input, client_point, keyboard, logical, modifiers, raw};
use crate::win32::window::frame;
use crate::win32::window::shared::Shared;

const MODAL_TIMER: usize = 1;
const SLOWEST_MODAL_TICK: Duration = Duration::from_millis(16);
const FASTEST_MODAL_TICK: Duration = Duration::from_millis(10);
const KEEP_FRAME: LPARAM = LPARAM(-1);
const BUTTON_DONE: LRESULT = LRESULT(1);

pub(crate) fn handle(
    shared: &Shared,
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    let id = shared.id;
    let custom = shared.frame.get().custom;
    if unsafe { rikka_appearance::is_appearance_change(message, wparam, lparam) } {
        heard::hear(id, Heard::Appearance);
        return None;
    }
    match message {
        WM_NCCALCSIZE if custom && wparam.0 != 0 => {
            Some(frame::calculate_size(shared, window, lparam))
        }
        WM_NCHITTEST if custom => frame::hit_test(shared, window, lparam),
        WM_NCACTIVATE if custom => {
            Some(unsafe { DefWindowProcW(window, message, wparam, KEEP_FRAME) })
        }
        WM_NCMOUSEMOVE | WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK | WM_NCLBUTTONUP
            if custom && frame::is_caption_button(wparam) =>
        {
            caption_button(shared, window, message, lparam)
        }
        WM_NCMOUSEMOVE => {
            with_input(shared, |input| input.mouse.left(id, true));
            None
        }
        WM_NCMOUSELEAVE => {
            with_input(shared, |input| input.mouse.left(id, true));
            None
        }
        WM_GETMINMAXINFO => Some(frame::limit(shared, window, lparam)),
        WM_DPICHANGED => {
            let suggested = unsafe { &*(lparam.0 as *const RECT) };
            if let Err(error) = unsafe {
                SetWindowPos(
                    window,
                    None,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            } {
                tracing::warn!("failed to follow the display scale: {error}");
            }
            heard::hear(id, Heard::Rescaled);
            Some(LRESULT(0))
        }
        WM_SIZE => {
            heard::hear(id, Heard::Resized);
            if shared.sizing.get() {
                modal_turn();
            }
            Some(LRESULT(0))
        }
        WM_ENTERSIZEMOVE => {
            shared.sizing.set(true);
            start_modal(window);
            None
        }
        WM_EXITSIZEMOVE => {
            shared.sizing.set(false);
            stop_modal(window);
            None
        }
        WM_ENTERMENULOOP => {
            start_modal(window);
            None
        }
        WM_EXITMENULOOP => {
            stop_modal(window);
            None
        }
        WM_TIMER if wparam.0 == MODAL_TIMER => {
            modal_turn();
            Some(LRESULT(0))
        }
        WM_CLOSE => {
            heard::hear(id, Heard::CloseRequested);
            Some(LRESULT(0))
        }
        WM_SETFOCUS => {
            heard::hear(id, Heard::Focus(true));
            None
        }
        WM_KILLFOCUS => {
            heard::hear(id, Heard::Focus(false));
            None
        }
        WM_DISPLAYCHANGE => {
            heard::hear(id, Heard::Displays);
            None
        }
        WM_ERASEBKGND => Some(LRESULT(1)),
        WM_SETCURSOR if (lparam.0 & 0xFFFF) as u32 == HTCLIENT => {
            show_pointer(shared.pointer.get());
            Some(LRESULT(1))
        }
        message if keyboard::is_key_message(message) => with_input(shared, |input| {
            input.keyboard.key(id, window, message, wparam, lparam)
        })
        .flatten(),
        WM_CHAR | WM_SYSCHAR => with_input(shared, |input| input.keyboard.character(id, wparam)),
        WM_MOUSEMOVE => {
            let (x, y) = client_point(lparam);
            let point = logical(window, x, y);
            with_input(shared, |input| input.mouse.moved(id, window, point));
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_input(shared, |input| input.mouse.left(id, false));
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN
        | WM_MBUTTONUP | WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let (button, pressed) = button_of(message, wparam);
            let (x, y) = client_point(lparam);
            let point = logical(window, x, y);
            with_input(shared, |input| {
                let modifiers = modifiers(input.keyboard.altgr());
                input
                    .mouse
                    .button(id, window, point, button, pressed, modifiers);
            });
            Some(match message {
                WM_XBUTTONDOWN | WM_XBUTTONUP => BUTTON_DONE,
                _ => LRESULT(0),
            })
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let delta = i32::from((wparam.0 >> 16) as u16 as i16);
            let screen = client_point(lparam);
            with_input(shared, |input| {
                let modifiers = modifiers(input.keyboard.altgr());
                input.mouse.wheel(
                    id,
                    window,
                    screen,
                    delta,
                    message == WM_MOUSEHWHEEL,
                    modifiers,
                );
            });
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED if lparam.0 as *mut core::ffi::c_void != window.0 => {
            with_input(shared, |input| input.mouse.capture_lost());
            Some(LRESULT(0))
        }
        WM_INPUT => {
            raw::travelled(id, window, lparam);
            None
        }
        WM_POINTERUPDATE
        | WM_POINTERDOWN
        | WM_POINTERUP
        | WM_POINTERENTER
        | WM_POINTERLEAVE
        | WM_POINTERCAPTURECHANGED => with_input(shared, |input| {
            input.pointers.handle(id, window, message, wparam)
        })
        .flatten(),
        WM_IME_SETCONTEXT => Some(Ime::set_context(window, message, wparam, lparam)),
        WM_IME_STARTCOMPOSITION => with_input(shared, |input| input.ime.started(window)),
        WM_IME_COMPOSITION => with_input(shared, |input| input.ime.composed(id, window, lparam)),
        WM_IME_ENDCOMPOSITION => with_input(shared, |input| input.ime.ended(id)),
        WM_IME_CHAR => Some(LRESULT(0)),
        _ => None,
    }
}

fn with_input<T>(shared: &Shared, act: impl FnOnce(&mut Input) -> T) -> Option<T> {
    match shared.input.try_borrow_mut() {
        Ok(mut input) => Some(act(&mut input)),
        Err(_) => {
            tracing::debug!("input arrived while input was being translated");
            None
        }
    }
}

fn caption_button(shared: &Shared, window: HWND, message: u32, lparam: LPARAM) -> Option<LRESULT> {
    let (x, y) = client_point(lparam);
    let mut at = POINT { x, y };
    if !unsafe { ScreenToClient(window, &mut at) }.as_bool() {
        return None;
    }
    let point = logical(window, at.x, at.y);
    let id = shared.id;
    with_input(shared, |input| {
        let modifiers = modifiers(input.keyboard.altgr());
        match message {
            WM_NCMOUSEMOVE => input.mouse.moved_over_frame(id, window, point),
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
                input
                    .mouse
                    .button(id, window, point, PointerButton::Primary, true, modifiers)
            }
            _ => input
                .mouse
                .button(id, window, point, PointerButton::Primary, false, modifiers),
        }
    })?;
    Some(LRESULT(0))
}

fn button_of(message: u32, wparam: WPARAM) -> (PointerButton, bool) {
    let side = match (wparam.0 >> 16) as u16 {
        XBUTTON1 => PointerButton::Back,
        _ => PointerButton::Forward,
    };
    match message {
        WM_LBUTTONDOWN => (PointerButton::Primary, true),
        WM_LBUTTONUP => (PointerButton::Primary, false),
        WM_RBUTTONDOWN => (PointerButton::Secondary, true),
        WM_RBUTTONUP => (PointerButton::Secondary, false),
        WM_MBUTTONDOWN => (PointerButton::Middle, true),
        WM_MBUTTONUP => (PointerButton::Middle, false),
        WM_XBUTTONDOWN => (side, true),
        _ => (side, false),
    }
}

pub(crate) fn show_pointer(pointer: Pointer) {
    match pointer {
        Pointer::Shown(cursor) => unsafe { SetCursor(Some(cursor)) },
        Pointer::Hidden => unsafe { SetCursor(None) },
    };
}

fn modal_tick() -> u32 {
    let period = RefreshRate::published_period().clamp(FASTEST_MODAL_TICK, SLOWEST_MODAL_TICK);
    period.as_millis() as u32
}

fn start_modal(window: HWND) {
    if unsafe { SetTimer(Some(window), MODAL_TIMER, modal_tick(), None) } == 0 {
        tracing::warn!(
            "failed to keep drawing during a modal loop: {}",
            windows::core::Error::from_thread()
        );
    }
}

fn stop_modal(window: HWND) {
    if let Err(error) = unsafe { KillTimer(Some(window), MODAL_TIMER) } {
        tracing::debug!("failed to stop the modal timer: {error}");
    }
}

fn modal_turn() {
    if with_runner(|runner| runner.modal_turn()).is_none() {
        tracing::trace!("modal turn skipped while the runner is busy");
    }
}
