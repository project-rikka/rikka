use std::ffi::{CStr, c_char};

use crate::ffi;

const UNKNOWN: &str = "unknown error";

pub(crate) fn owned(text: *mut c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    let copied = unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned();
    unsafe { ffi::ZXing_free(text.cast()) };
    Some(copied)
}

pub(crate) fn last() -> String {
    described(unsafe { ffi::ZXing_LastErrorMsg() })
}

pub(crate) fn described(text: *mut c_char) -> String {
    owned(text)
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| UNKNOWN.to_owned())
}
