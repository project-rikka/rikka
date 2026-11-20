use rikka_theme::winui::{SystemColors, from_colorref};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, COLOR_BTNTEXT, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT,
    COLOR_HOTLIGHT, COLOR_WINDOW, COLOR_WINDOWTEXT, GetSysColor, SYS_COLOR_INDEX,
};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

pub(crate) fn high_contrast() -> Option<SystemColors> {
    let mut info = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        ..HIGHCONTRASTW::default()
    };
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            info.cbSize,
            Some((&raw mut info).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if let Err(error) = read {
        tracing::warn!("failed to read the high contrast setting: {error}");
        return None;
    }
    info.dwFlags
        .contains(HCF_HIGHCONTRASTON)
        .then(system_colors)
}

fn system_colors() -> SystemColors {
    let color = |index: SYS_COLOR_INDEX| from_colorref(unsafe { GetSysColor(index) });
    SystemColors {
        window: color(COLOR_WINDOW),
        window_text: color(COLOR_WINDOWTEXT),
        highlight: color(COLOR_HIGHLIGHT),
        highlight_text: color(COLOR_HIGHLIGHTTEXT),
        hotlight: color(COLOR_HOTLIGHT),
        gray_text: color(COLOR_GRAYTEXT),
        button_face: color(COLOR_BTNFACE),
        button_text: color(COLOR_BTNTEXT),
    }
}
