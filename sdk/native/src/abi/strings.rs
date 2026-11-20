use std::ffi::{CStr, c_char};

use crate::abi::violation;
use crate::ffi;

pub(crate) unsafe fn text<'a>(raw: ffi::Str, param: &str) -> &'a str {
    let bytes: &[u8] = match (raw.length, raw.data.is_null()) {
        (0, _) => &[],
        (_, true) => violation(format!("`{param}` has NULL data and a nonzero length")),
        (ffi::STRLEN, false) => unsafe { CStr::from_ptr(raw.data) }.to_bytes(),
        (length, false) => unsafe { std::slice::from_raw_parts(raw.data.cast::<u8>(), length) },
    };
    std::str::from_utf8(bytes)
        .unwrap_or_else(|error| violation(format!("`{param}` is not UTF-8: {error}")))
}

pub(crate) unsafe fn slice<'a, T>(raw: *const T, count: usize, param: &str) -> &'a [T] {
    match (count, raw.is_null()) {
        (0, _) => &[],
        (_, true) => violation(format!("`{param}` is NULL with a nonzero count")),
        (count, false) => unsafe { std::slice::from_raw_parts(raw, count) },
    }
}

pub(crate) fn view(text: &str) -> ffi::Str {
    ffi::Str {
        data: text.as_ptr().cast::<c_char>(),
        length: text.len(),
    }
}

pub(crate) fn views(texts: &[&str]) -> Vec<ffi::Str> {
    texts.iter().map(|text| view(text)).collect()
}

pub(crate) fn give_string(text: String) -> ffi::String {
    let mut bytes = text.into_bytes();
    let length = bytes.len();
    bytes.push(0);
    let data = Box::into_raw(bytes.into_boxed_slice()).cast::<c_char>();
    ffi::String { data, length }
}

pub(crate) unsafe fn free_string(string: ffi::String) {
    if string.data.is_null() {
        return;
    }
    let bytes = std::ptr::slice_from_raw_parts_mut(string.data.cast::<u8>(), string.length + 1);
    drop(unsafe { Box::from_raw(bytes) });
}
