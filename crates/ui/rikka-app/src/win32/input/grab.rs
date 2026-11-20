use std::cell::Cell;

use rikka_input::win32::{key_code, location, named_key};
use rikka_types::Key;
use rikka_ui::Event;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_LWIN, VK_RWIN, VK_TAB};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, LLKHF_ALTDOWN,
    LLKHF_EXTENDED, LLKHF_UP, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
};

use crate::tasks::WindowId;
use crate::win32::heard;
use crate::win32::input::modifiers;

thread_local! {
    static GRABBING: Cell<Option<(HWND, WindowId)>> = const { Cell::new(None) };
    static LOGO: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug)]
pub(crate) struct KeyboardGrab {
    hook: HHOOK,
}

impl KeyboardGrab {
    pub(crate) fn hold(window: HWND, id: WindowId) -> Option<KeyboardGrab> {
        match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hooked), None, 0) } {
            Ok(hook) => {
                GRABBING.set(Some((window, id)));
                Some(KeyboardGrab { hook })
            }
            Err(error) => {
                tracing::warn!("failed to grab the keyboard: {error}");
                None
            }
        }
    }
}

impl Drop for KeyboardGrab {
    fn drop(&mut self) {
        GRABBING.set(None);
        LOGO.set(false);
        if let Err(error) = unsafe { UnhookWindowsHookEx(self.hook) } {
            tracing::debug!("failed to release the keyboard: {error}");
        }
    }
}

pub(crate) fn holds_logo() -> bool {
    LOGO.get()
}

pub(crate) fn grabs(window: HWND) -> bool {
    GRABBING
        .get()
        .is_some_and(|(grabbing, _)| grabbing == window)
}

unsafe extern "system" fn hooked(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32
        && let Some((window, id)) = GRABBING.get()
        && unsafe { GetForegroundWindow() } == window
    {
        let stroke = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if reserved_by_system(stroke) {
            deliver(id, stroke);
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn reserved_by_system(stroke: &KBDLLHOOKSTRUCT) -> bool {
    let alt = stroke.flags.0 & LLKHF_ALTDOWN.0 != 0;
    let control = modifiers(false).control;
    match stroke.vkCode as u16 {
        key if key == VK_LWIN.0 || key == VK_RWIN.0 => true,
        key if key == VK_TAB.0 => alt,
        key if key == VK_ESCAPE.0 => alt || control,
        _ => false,
    }
}

fn deliver(id: WindowId, stroke: &KBDLLHOOKSTRUCT) {
    let virtual_key = stroke.vkCode as u16;
    let scan = (stroke.scanCode & 0xFF) as u16;
    let extended = stroke.flags.0 & LLKHF_EXTENDED.0 != 0;
    let released = stroke.flags.0 & LLKHF_UP.0 != 0;
    if virtual_key == VK_LWIN.0 || virtual_key == VK_RWIN.0 {
        LOGO.set(!released);
    }
    let key = named_key(virtual_key, scan, extended).unwrap_or(Key::Unidentified);
    let mut modifiers = modifiers(false);
    modifiers.alt |= stroke.flags.0 & LLKHF_ALTDOWN.0 != 0;
    let code = key_code(scan, extended);
    let location = location(scan, extended);
    let event = match released {
        false => Event::KeyPressed {
            key,
            code,
            location,
            modifiers,
        },
        true => Event::KeyReleased {
            key,
            code,
            location,
            modifiers,
        },
    };
    heard::input(id, event);
}
