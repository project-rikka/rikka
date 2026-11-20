use rikka_types::{Point, ResizeEdge, Size};
use rikka_ui::WindowPart;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, ScreenToClient,
};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{
    ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_GETAUTOHIDEBAREX, ABM_GETSTATE, ABS_AUTOHIDE,
    APPBARDATA, SHAppBarMessage,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetWindowRect, HTBORDER, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION,
    HTCLIENT, HTCLOSE, HTLEFT, HTMAXBUTTON, HTMINBUTTON, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
    IsZoomed, MINMAXINFO, NCCALCSIZE_PARAMS, SM_CXFRAME, SM_CXPADDEDBORDER, SM_CYFRAME,
};

use crate::win32::active::with_runner;
use crate::win32::input::{client_point, scale_of};
use crate::win32::window::shared::Shared;

const AUTOHIDE_REVEAL: i32 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Insets {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

pub(crate) fn thickness(dpi: u32) -> (i32, i32) {
    let padding = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    let x = unsafe { GetSystemMetricsForDpi(SM_CXFRAME, dpi) } + padding;
    let y = unsafe { GetSystemMetricsForDpi(SM_CYFRAME, dpi) } + padding;
    (x, y)
}

pub(crate) fn is_maximized(window: HWND) -> bool {
    unsafe { IsZoomed(window) }.as_bool()
}

pub(crate) fn insets(shared: &Shared, window: HWND) -> Insets {
    let frame = shared.frame.get();
    if frame.fullscreen {
        return Insets::default();
    }
    let (x, y) = thickness(unsafe { GetDpiForWindow(window) });
    match is_maximized(window) {
        true => Insets {
            left: x,
            top: y,
            right: x,
            bottom: y,
        },
        false => Insets {
            left: x,
            top: 0,
            right: x,
            bottom: y,
        },
    }
}

pub(crate) fn calculate_size(shared: &Shared, window: HWND, lparam: LPARAM) -> LRESULT {
    let insets = insets(shared, window);
    let proposed = unsafe { &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS) };
    let rect = &mut proposed.rgrc[0];
    rect.left += insets.left;
    rect.top += insets.top;
    rect.right -= insets.right;
    rect.bottom -= insets.bottom;
    if is_maximized(window) && !shared.frame.get().fullscreen {
        reveal_hidden_taskbar(window, rect);
    }
    LRESULT(0)
}

fn reveal_hidden_taskbar(window: HWND, rect: &mut RECT) {
    let mut query = APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        ..APPBARDATA::default()
    };
    let state = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut query) } as u32;
    if state & ABS_AUTOHIDE == 0 {
        return;
    }
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return;
    }
    for edge in [ABE_BOTTOM, ABE_TOP, ABE_LEFT, ABE_RIGHT] {
        let mut bar = APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            uEdge: edge,
            rc: info.rcMonitor,
            ..APPBARDATA::default()
        };
        if unsafe { SHAppBarMessage(ABM_GETAUTOHIDEBAREX, &mut bar) } == 0 {
            continue;
        }
        match edge {
            ABE_BOTTOM => rect.bottom -= AUTOHIDE_REVEAL,
            ABE_TOP => rect.top += AUTOHIDE_REVEAL,
            ABE_LEFT => rect.left += AUTOHIDE_REVEAL,
            _ => rect.right -= AUTOHIDE_REVEAL,
        }
        return;
    }
}

pub(crate) fn hit_test(shared: &Shared, window: HWND, lparam: LPARAM) -> Option<LRESULT> {
    let (x, y) = client_point(lparam);
    let mut at = POINT { x, y };
    if !unsafe { ScreenToClient(window, &mut at) }.as_bool() {
        return None;
    }
    let frame = shared.frame.get();
    let floating = !frame.fullscreen && !is_maximized(window);
    let sizable = floating && frame.resizable;
    let mut client = RECT::default();
    if let Err(error) = unsafe { GetClientRect(window, &mut client) } {
        tracing::debug!("failed to measure the client area: {error}");
        return None;
    }
    if at.x < 0 || at.y < 0 || at.x >= client.right || at.y >= client.bottom {
        return match sizable {
            true => None,
            false => Some(LRESULT(HTBORDER as isize)),
        };
    }
    if sizable {
        let (border_x, border_y) = thickness(unsafe { GetDpiForWindow(window) });
        if at.y < border_y {
            let code = match at.x {
                x if x < border_x * 2 => HTTOPLEFT,
                x if x >= client.right - border_x * 2 => HTTOPRIGHT,
                _ => HTTOP,
            };
            return Some(LRESULT(code as isize));
        }
    }
    let scale = scale_of(window);
    let point = Point::new(
        (f64::from(at.x) / scale) as f32,
        (f64::from(at.y) / scale) as f32,
    );
    let part = with_runner(|runner| runner.window_part(shared.id, point)).flatten();
    let code = match part {
        Some(WindowPart::Caption) => HTCAPTION,
        Some(WindowPart::Minimize) => HTMINBUTTON,
        Some(WindowPart::Maximize) => HTMAXBUTTON,
        Some(WindowPart::Close) => HTCLOSE,
        Some(WindowPart::Resize(edge)) if floating && frame.resizable => edge_code(edge),
        Some(WindowPart::Resize(_)) | None => HTCLIENT,
    };
    Some(LRESULT(code as isize))
}

pub(crate) fn edge_code(edge: ResizeEdge) -> u32 {
    match edge {
        ResizeEdge::Top => HTTOP,
        ResizeEdge::Bottom => HTBOTTOM,
        ResizeEdge::Left => HTLEFT,
        ResizeEdge::Right => HTRIGHT,
        ResizeEdge::TopLeft => HTTOPLEFT,
        ResizeEdge::TopRight => HTTOPRIGHT,
        ResizeEdge::BottomLeft => HTBOTTOMLEFT,
        ResizeEdge::BottomRight => HTBOTTOMRIGHT,
    }
}

pub(crate) fn is_caption_button(code: WPARAM) -> bool {
    matches!(code.0 as u32, HTMINBUTTON | HTMAXBUTTON | HTCLOSE)
}

pub(crate) fn limit(shared: &Shared, window: HWND, lparam: LPARAM) -> LRESULT {
    let frame = shared.frame.get();
    let limits = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
    let Some(min) = frame.min_size else {
        return LRESULT(0);
    };
    let outer = outer_size(shared, window, min);
    limits.ptMinTrackSize = POINT {
        x: outer.0.max(limits.ptMinTrackSize.x),
        y: outer.1.max(limits.ptMinTrackSize.y),
    };
    LRESULT(0)
}

pub(crate) fn outer_size(shared: &Shared, window: HWND, client: Size) -> (i32, i32) {
    let scale = scale_of(window);
    let insets = match shared.frame.get().custom {
        true => {
            let (x, y) = thickness(unsafe { GetDpiForWindow(window) });
            Insets {
                left: x,
                top: 0,
                right: x,
                bottom: y,
            }
        }
        false => standard_insets(window),
    };
    (
        (f64::from(client.width) * scale).round() as i32 + insets.left + insets.right,
        (f64::from(client.height) * scale).round() as i32 + insets.top + insets.bottom,
    )
}

fn standard_insets(window: HWND) -> Insets {
    let mut outer = RECT::default();
    let mut inner = RECT::default();
    let read = unsafe {
        GetWindowRect(window, &mut outer).and_then(|()| GetClientRect(window, &mut inner))
    };
    if read.is_err() {
        return Insets::default();
    }
    let horizontal = (outer.right - outer.left) - inner.right;
    let vertical = (outer.bottom - outer.top) - inner.bottom;
    Insets {
        left: horizontal / 2,
        top: vertical - horizontal / 2,
        right: horizontal - horizontal / 2,
        bottom: horizontal / 2,
    }
}
