use core::ffi::c_int;

use crate::avutil;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{context}: {message} (AVERROR {code})")]
    Av {
        context: &'static str,
        code: i32,
        message: String,
    },

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("FFmpeg is unavailable: {0}")]
    Unavailable(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub fn av_err(context: &'static str, code: c_int) -> Error {
    Error::Av {
        context,
        code,
        message: describe(code),
    }
}

pub fn check(context: &'static str, code: c_int) -> Result<c_int> {
    match code >= 0 {
        true => Ok(code),
        false => Err(av_err(context, code)),
    }
}

pub fn describe(code: c_int) -> String {
    let mut buffer = [0u8; avutil::AV_ERROR_MAX_STRING_SIZE];
    unsafe {
        if avutil::av_strerror(code, buffer.as_mut_ptr().cast(), buffer.len()) == 0 {
            core::ffi::CStr::from_bytes_until_nul(&buffer)
                .map(|text| text.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            format!("unknown error {code}")
        }
    }
}
