use crate::error;
use crate::ffi;

pub(crate) fn version() -> u32 {
    ffi::VERSION
}

pub(crate) fn last_error() -> ffi::Str {
    error::last()
}
