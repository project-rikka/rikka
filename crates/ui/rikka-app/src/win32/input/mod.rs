pub(crate) mod cursor;
pub(crate) mod grab;
pub(crate) mod ime;
pub(crate) mod keyboard;
pub(crate) mod mouse;
pub(crate) mod pointer;
pub(crate) mod raw;

use rikka_types::{Modifiers, Point};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};

const BASE_DPI: f64 = 96.0;

#[derive(Debug, Default)]
pub(crate) struct Input {
    pub(crate) keyboard: keyboard::Keyboard,
    pub(crate) mouse: mouse::Mouse,
    pub(crate) pointers: pointer::Pointers,
    pub(crate) ime: ime::Ime,
}

pub(crate) fn scale_of(window: HWND) -> f64 {
    match unsafe { GetDpiForWindow(window) } {
        0 => 1.0,
        dpi => f64::from(dpi) / BASE_DPI,
    }
}

pub(crate) fn logical(window: HWND, x: i32, y: i32) -> Point {
    let scale = scale_of(window);
    Point::new((f64::from(x) / scale) as f32, (f64::from(y) / scale) as f32)
}

pub(crate) fn client_point(lparam: LPARAM) -> (i32, i32) {
    let packed = lparam.0 as u32;
    (
        i32::from((packed & 0xFFFF) as u16 as i16),
        i32::from((packed >> 16) as u16 as i16),
    )
}

pub(crate) fn is_down(key: VIRTUAL_KEY) -> bool {
    (unsafe { GetKeyState(i32::from(key.0)) }) < 0
}

pub(crate) fn modifiers(altgr: bool) -> Modifiers {
    Modifiers {
        shift: is_down(VK_SHIFT),
        control: is_down(VK_CONTROL) && !altgr,
        alt: is_down(VK_MENU) && !altgr,
        logo: is_down(VK_LWIN) || is_down(VK_RWIN) || grab::holds_logo(),
    }
}
