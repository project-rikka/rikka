use rikka_types::{Point, ResizeEdge};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    EnableMenuItem, GWL_STYLE, GetCursorPos, GetSystemMenu, GetWindowLongPtrW, GetWindowPlacement,
    HWND_TOP, MF_BYCOMMAND, MF_ENABLED, MF_GRAYED, PostMessageW, SC_MAXIMIZE, SC_MINIMIZE, SC_MOVE,
    SC_RESTORE, SC_SIZE, SHOW_WINDOW_CMD, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOOWNERZORDER,
    SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetWindowLongPtrW, SetWindowPlacement, SetWindowPos,
    ShowWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WINDOWPLACEMENT, WM_SYSCOMMAND,
    WS_CAPTION, WS_THICKFRAME,
};

use crate::win32::input::scale_of;

const DRAGGED_BY_CAPTION: usize = 2;
const MISSING_MENU_ITEM: i32 = -1;

pub(crate) fn show(window: HWND, command: SHOW_WINDOW_CMD) {
    let _was_visible = unsafe { ShowWindow(window, command) };
}

#[derive(Debug)]
pub(crate) struct Restore {
    style: isize,
    placement: WINDOWPLACEMENT,
}

pub(crate) fn enter_fullscreen(window: HWND) -> Option<Restore> {
    let mut placement = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        ..WINDOWPLACEMENT::default()
    };
    if let Err(error) = unsafe { GetWindowPlacement(window, &mut placement) } {
        tracing::warn!("failed to remember the window placement: {error}");
        return None;
    }
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        tracing::warn!("failed to read the display of the window");
        return None;
    }
    let style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) };
    let framed = (WS_CAPTION.0 | WS_THICKFRAME.0) as isize;
    unsafe { SetWindowLongPtrW(window, GWL_STYLE, style & !framed) };
    let screen = info.rcMonitor;
    if let Err(error) = unsafe {
        SetWindowPos(
            window,
            Some(HWND_TOP),
            screen.left,
            screen.top,
            screen.right - screen.left,
            screen.bottom - screen.top,
            SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
        )
    } {
        tracing::warn!("failed to fill the display: {error}");
    }
    Some(Restore { style, placement })
}

pub(crate) fn leave_fullscreen(window: HWND, restore: &Restore) {
    unsafe { SetWindowLongPtrW(window, GWL_STYLE, restore.style) };
    if let Err(error) = unsafe { SetWindowPlacement(window, &restore.placement) } {
        tracing::warn!("failed to restore the window placement: {error}");
    }
    if let Err(error) = unsafe {
        SetWindowPos(
            window,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
        )
    } {
        tracing::warn!("failed to restore the window frame: {error}");
    }
}

pub(crate) fn start_move(window: HWND) {
    system_drag(window, SC_MOVE as usize | DRAGGED_BY_CAPTION);
}

pub(crate) fn start_resize(window: HWND, edge: ResizeEdge) {
    let side = match edge {
        ResizeEdge::Left => 1,
        ResizeEdge::Right => 2,
        ResizeEdge::Top => 3,
        ResizeEdge::TopLeft => 4,
        ResizeEdge::TopRight => 5,
        ResizeEdge::Bottom => 6,
        ResizeEdge::BottomLeft => 7,
        ResizeEdge::BottomRight => 8,
    };
    system_drag(window, SC_SIZE as usize | side);
}

fn system_drag(window: HWND, command: usize) {
    let mut at = POINT::default();
    if let Err(error) = unsafe { GetCursorPos(&mut at) } {
        tracing::debug!("failed to read the pointer position: {error}");
    }
    if let Err(error) = unsafe { ReleaseCapture() } {
        tracing::debug!("failed to release pointer capture: {error}");
    }
    let packed = (at.x as u16 as isize) | ((at.y as u16 as isize) << 16);
    unsafe {
        SendMessageW(
            window,
            WM_SYSCOMMAND,
            Some(WPARAM(command)),
            Some(LPARAM(packed)),
        )
    };
}

pub(crate) fn show_menu(window: HWND, at: Point, maximized: bool, resizable: bool) {
    let scale = scale_of(window);
    let mut screen = POINT {
        x: (f64::from(at.x) * scale).round() as i32,
        y: (f64::from(at.y) * scale).round() as i32,
    };
    if !unsafe { ClientToScreen(window, &mut screen) }.as_bool() {
        return;
    }
    let menu = unsafe { GetSystemMenu(window, false) };
    if menu.is_invalid() {
        tracing::debug!("window has no system menu");
        return;
    }
    let state = |enabled: bool| match enabled {
        true => MF_BYCOMMAND | MF_ENABLED,
        false => MF_BYCOMMAND | MF_GRAYED,
    };
    for (command, enabled) in [
        (SC_RESTORE, maximized),
        (SC_MOVE, !maximized),
        (SC_SIZE, !maximized && resizable),
        (SC_MINIMIZE, true),
        (SC_MAXIMIZE, !maximized && resizable),
    ] {
        if unsafe { EnableMenuItem(menu, command, state(enabled)) }.0 == MISSING_MENU_ITEM {
            tracing::debug!("window menu lacks command {command:#x}");
        }
    }
    let chosen = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            screen.x,
            screen.y,
            None,
            window,
            None,
        )
    };
    if chosen.0 != 0
        && let Err(error) = unsafe {
            PostMessageW(
                Some(window),
                WM_SYSCOMMAND,
                WPARAM(chosen.0 as usize),
                LPARAM(0),
            )
        }
    {
        tracing::warn!("failed to run the window menu command: {error}");
    }
}
