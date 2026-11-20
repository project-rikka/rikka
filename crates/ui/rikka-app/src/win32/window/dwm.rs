use std::ffi::c_void;

use rikka_theme::Theme;
use rikka_types::Color;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMWINDOWATTRIBUTE, DwmSetWindowAttribute,
};
use windows::core::BOOL;

const LEGACY_DARK_MODE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(19);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Look {
    pub(crate) dark: bool,
    pub(crate) border: Color,
}

impl Look {
    pub(crate) fn of(theme: &Theme) -> Look {
        Look {
            dark: theme.is_dark(),
            border: theme.palette().outline,
        }
    }
}

pub(crate) fn apply(window: HWND, look: Look) {
    let dark = BOOL::from(look.dark);
    if set(window, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark).is_err()
        && let Err(error) = set(window, LEGACY_DARK_MODE, &dark)
    {
        tracing::debug!("dark window frames unsupported: {error}");
    }
    optional(set::<DWM_WINDOW_CORNER_PREFERENCE>(
        window,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &DWMWCP_ROUND,
    ));
    optional(set(window, DWMWA_BORDER_COLOR, &color_ref(look.border)));
}

fn optional(result: windows::core::Result<()>) {
    if let Err(error) = result {
        tracing::debug!("window attribute unsupported by this windows version: {error}");
    }
}

fn set<T>(window: HWND, attribute: DWMWINDOWATTRIBUTE, value: &T) -> windows::core::Result<()> {
    unsafe {
        DwmSetWindowAttribute(
            window,
            attribute,
            (value as *const T).cast::<c_void>(),
            size_of::<T>() as u32,
        )
    }
}

fn color_ref(color: Color) -> u32 {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    channel(color.r) | (channel(color.g) << 8) | (channel(color.b) << 16)
}
