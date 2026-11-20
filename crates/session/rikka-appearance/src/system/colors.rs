use rikka_theme::Variant;
use rikka_theme::winui::{AccentPalette, from_colorref};
use rikka_types::Color;
use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::System::Registry::HKEY_CURRENT_USER;
use windows::core::{Result, w};

use crate::system::registry::Value;

const APPS_USE_LIGHT_THEME: Value = Value {
    root: HKEY_CURRENT_USER,
    key: w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
    name: w!("AppsUseLightTheme"),
};

const ACCENT_PALETTE: Value = Value {
    root: HKEY_CURRENT_USER,
    key: w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent"),
    name: w!("AccentPalette"),
};

const DWM_ACCENT_COLOR: Value = Value {
    root: HKEY_CURRENT_USER,
    key: w!(r"Software\Microsoft\Windows\DWM"),
    name: w!("AccentColor"),
};

pub(crate) fn variant(settings: Option<&UISettings>) -> Variant {
    if let Some(settings) = settings {
        match settings.GetColorValue(UIColorType::Foreground) {
            Ok(foreground) => return variant_under(foreground),
            Err(error) => tracing::warn!("failed to read the system foreground colour: {error}"),
        }
    }
    match APPS_USE_LIGHT_THEME.dword() {
        Ok(Some(0)) => Variant::Dark,
        Ok(Some(_) | None) => Variant::Light,
        Err(error) => {
            tracing::warn!("failed to read the system colour mode: {error}");
            Variant::Light
        }
    }
}

fn variant_under(foreground: windows::UI::Color) -> Variant {
    let weight =
        5 * u32::from(foreground.G) + 2 * u32::from(foreground.R) + u32::from(foreground.B);
    match weight > 8 * 128 {
        true => Variant::Dark,
        false => Variant::Light,
    }
}

pub(crate) fn accent(settings: Option<&UISettings>) -> AccentPalette {
    if let Some(settings) = settings {
        match system_palette(settings) {
            Ok(palette) => return palette,
            Err(error) => tracing::warn!("failed to read the system accent colours: {error}"),
        }
    }
    match ACCENT_PALETTE.binary() {
        Ok(Some(bytes)) => match AccentPalette::from_registry(&bytes) {
            Some(palette) => return palette,
            None => tracing::warn!("system accent palette has {} bytes", bytes.len()),
        },
        Ok(None) => {}
        Err(error) => tracing::warn!("failed to read the system accent palette: {error}"),
    }
    match DWM_ACCENT_COLOR.dword() {
        Ok(Some(value)) => return AccentPalette::from_accent(from_colorref(value)),
        Ok(None) => {}
        Err(error) => tracing::warn!("failed to read the window accent colour: {error}"),
    }
    tracing::warn!("system reports no accent colour, using the windows default");
    AccentPalette::WINDOWS_DEFAULT
}

fn system_palette(settings: &UISettings) -> Result<AccentPalette> {
    let shade = |kind: UIColorType| {
        settings
            .GetColorValue(kind)
            .map(|color| Color::rgb8(color.R, color.G, color.B))
    };
    Ok(AccentPalette {
        accent: shade(UIColorType::Accent)?,
        light1: shade(UIColorType::AccentLight1)?,
        light2: shade(UIColorType::AccentLight2)?,
        light3: shade(UIColorType::AccentLight3)?,
        dark1: shade(UIColorType::AccentDark1)?,
        dark2: shade(UIColorType::AccentDark2)?,
        dark3: shade(UIColorType::AccentDark3)?,
    })
}
