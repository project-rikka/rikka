use rikka_ui::TextRun;

use crate::abi::violation;
use crate::error;
use crate::ffi;

pub(crate) fn version() -> u32 {
    ffi::VERSION
}

pub(crate) fn last_error() -> ffi::Str {
    error::last()
}

pub(crate) fn text_width(text: &str, size: f32) -> f32 {
    if !(size.is_finite() && size > 0.0) {
        violation(format!("{size} is not a text size"));
    }
    rikka_ui::text_width(&TextRun::new(text, size))
}
