use windows::Win32::Security::Authentication::Identity::{
    EXTENDED_NAME_FORMAT, GetUserNameExW, NameDisplay, NameSamCompatible,
};
use windows::core::PWSTR;

use super::Entry;

#[must_use]
pub fn current() -> Option<Entry> {
    let qualified = user_name(NameSamCompatible)?;
    let name = match qualified.rsplit_once('\\') {
        Some((_, account)) => account.to_owned(),
        None => qualified,
    };
    Some(Entry {
        name,
        gecos: user_name(NameDisplay).unwrap_or_default(),
        home: crate::home()?.to_path_buf(),
    })
}

fn user_name(format: EXTENDED_NAME_FORMAT) -> Option<String> {
    let mut size = 0u32;
    let _ = unsafe { GetUserNameExW(format, None, &mut size) };
    if size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; size as usize];
    let written = unsafe { GetUserNameExW(format, Some(PWSTR(buffer.as_mut_ptr())), &mut size) };
    let length = (size as usize).min(buffer.len());
    written.then(|| String::from_utf16_lossy(&buffer[..length]))
}
