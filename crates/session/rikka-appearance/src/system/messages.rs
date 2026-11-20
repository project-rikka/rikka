use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_SETHIGHCONTRAST, WM_DWMCOLORIZATIONCOLORCHANGED, WM_SETTINGCHANGE, WM_SYSCOLORCHANGE,
    WM_THEMECHANGED,
};
use windows::core::PCWSTR;

const IMMERSIVE_COLOR_SET: &str = "ImmersiveColorSet";

pub unsafe fn is_appearance_change(message: u32, wparam: WPARAM, lparam: LPARAM) -> bool {
    match message {
        WM_DWMCOLORIZATIONCOLORCHANGED | WM_SYSCOLORCHANGE | WM_THEMECHANGED => true,
        WM_SETTINGCHANGE => {
            wparam.0 == SPI_SETHIGHCONTRAST.0 as usize || unsafe { names_color_set(lparam) }
        }
        _ => false,
    }
}

unsafe fn names_color_set(lparam: LPARAM) -> bool {
    let area = PCWSTR(lparam.0 as *const u16);
    if area.is_null() {
        return false;
    }
    unsafe { area.as_wide() }
        .iter()
        .copied()
        .eq(IMMERSIVE_COLOR_SET.encode_utf16())
}
