use windows::Win32::Globalization::{
    GetUserDefaultLocaleName, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME,
};
use windows::core::PWSTR;

const LOCALE_NAME_MAX_LENGTH: usize = 85;

pub(crate) fn tag() -> Option<String> {
    preferred_ui_language().or_else(default_locale_name)
}

fn preferred_ui_language() -> Option<String> {
    let mut count = 0u32;
    let mut length = 0u32;
    unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut length) }
        .ok()?;
    let mut buffer = vec![0u16; length as usize];
    unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
    }
    .ok()?;
    buffer
        .split(|unit| *unit == 0)
        .find(|name| !name.is_empty())
        .map(String::from_utf16_lossy)
}

fn default_locale_name() -> Option<String> {
    let mut buffer = [0u16; LOCALE_NAME_MAX_LENGTH];
    let written = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    let length = usize::try_from(written).ok()?.checked_sub(1)?;
    (length > 0).then(|| String::from_utf16_lossy(&buffer[..length]))
}
