use rikka_types::Point;
use rikka_ui::Event;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::UI::Input::{
    GetRawInputData, HRAWINPUT, MOUSE_MOVE_ABSOLUTE, RAWINPUT, RAWINPUTDEVICE,
    RAWINPUTDEVICE_FLAGS, RAWINPUTHEADER, RID_INPUT, RIDEV_REMOVE, RIM_TYPEMOUSE,
    RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{ClipCursor, GetCursorPos};

use crate::tasks::WindowId;
use crate::win32::heard;
use crate::win32::input::scale_of;

const GENERIC_PAGE: u16 = 0x01;
const MOUSE_USAGE: u16 = 0x02;

#[derive(Debug)]
pub(crate) struct PointerLock {
    clipped: bool,
}

impl PointerLock {
    pub(crate) fn hold(window: HWND) -> Option<PointerLock> {
        let device = RAWINPUTDEVICE {
            usUsagePage: GENERIC_PAGE,
            usUsage: MOUSE_USAGE,
            dwFlags: RAWINPUTDEVICE_FLAGS(0),
            hwndTarget: window,
        };
        if let Err(error) =
            unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) }
        {
            tracing::warn!("failed to lock pointer: {error}");
            return None;
        }
        Some(PointerLock { clipped: false })
    }

    pub(crate) fn follow_focus(&mut self, focused: bool) {
        if focused == self.clipped {
            return;
        }
        let clip = match focused {
            true => {
                let mut at = POINT::default();
                if let Err(error) = unsafe { GetCursorPos(&mut at) } {
                    tracing::debug!("failed to read pointer position: {error}");
                    return;
                }
                Some(RECT {
                    left: at.x,
                    top: at.y,
                    right: at.x + 1,
                    bottom: at.y + 1,
                })
            }
            false => None,
        };
        let result = match clip.as_ref() {
            Some(rect) => unsafe { ClipCursor(Some(rect)) },
            None => unsafe { ClipCursor(None) },
        };
        match result {
            Ok(()) => self.clipped = focused,
            Err(error) => tracing::debug!("failed to confine pointer: {error}"),
        }
    }
}

impl Drop for PointerLock {
    fn drop(&mut self) {
        if self.clipped
            && let Err(error) = unsafe { ClipCursor(None) }
        {
            tracing::debug!("failed to free pointer: {error}");
        }
        let device = RAWINPUTDEVICE {
            usUsagePage: GENERIC_PAGE,
            usUsage: MOUSE_USAGE,
            dwFlags: RIDEV_REMOVE,
            hwndTarget: HWND::default(),
        };
        if let Err(error) =
            unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) }
        {
            tracing::debug!("failed to stop relative pointer input: {error}");
        }
    }
}

pub(crate) fn travelled(id: WindowId, window: HWND, lparam: LPARAM) {
    let mut input = RAWINPUT::default();
    let mut size = size_of::<RAWINPUT>() as u32;
    let read = unsafe {
        GetRawInputData(
            HRAWINPUT(lparam.0 as *mut _),
            RID_INPUT,
            Some((&raw mut input).cast()),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if read == u32::MAX || input.header.dwType != RIM_TYPEMOUSE.0 {
        return;
    }
    let mouse = unsafe { input.data.mouse };
    if mouse.usFlags.0 & MOUSE_MOVE_ABSOLUTE.0 != 0 || (mouse.lLastX == 0 && mouse.lLastY == 0) {
        return;
    }
    let scale = scale_of(window);
    heard::input(
        id,
        Event::PointerTravelled(Point::new(
            (f64::from(mouse.lLastX) / scale) as f32,
            (f64::from(mouse.lLastY) / scale) as f32,
        )),
    );
}
