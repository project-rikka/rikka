use std::ffi::{c_char, c_int, c_void};
use std::path::Path;

use crate::error::{Error, Result};

pub(crate) type Session = *mut c_void;

#[repr(C)]
pub(crate) struct ModelInfo {
    pub(crate) path: *const libc::wchar_t,
    pub(crate) blob: *const c_void,
    pub(crate) blob_size: usize,
}

#[repr(C)]
pub(crate) struct NcConfig {
    pub(crate) input_sample_rate: c_int,
    pub(crate) input_frame_duration: c_int,
    pub(crate) output_sample_rate: c_int,
    pub(crate) model: *const ModelInfo,
    pub(crate) session_stats: bool,
    pub(crate) ringtone: *const c_void,
}

#[repr(C)]
pub(crate) struct VadConfig {
    pub(crate) input_sample_rate: c_int,
    pub(crate) input_frame_duration: c_int,
    pub(crate) model: *const ModelInfo,
}

pub(crate) type LogCallback = unsafe extern "C" fn(*const c_char, c_int);
pub(crate) type GlobalInit =
    unsafe extern "C" fn(*const libc::wchar_t, Option<LogCallback>, c_int) -> c_int;
pub(crate) type CreateNc = unsafe extern "C" fn(*const NcConfig) -> Session;
pub(crate) type ProcessNc =
    unsafe extern "C" fn(Session, *const f32, usize, *mut f32, usize, f32, *mut c_void) -> c_int;
pub(crate) type CreateVad = unsafe extern "C" fn(*const VadConfig) -> Session;
pub(crate) type ProcessVad = unsafe extern "C" fn(Session, *const f32, usize, *mut f32) -> c_int;
pub(crate) type Destroy = unsafe extern "C" fn(Session) -> c_int;

pub(crate) const LOG_OFF: c_int = 0;
pub(crate) const FRAME_MS: c_int = 10;

pub(crate) struct WidePath(Vec<libc::wchar_t>);

impl WidePath {
    pub(crate) fn new(path: &Path) -> Result<WidePath> {
        let text = path.to_str().ok_or_else(|| Error::Incomplete {
            folder: path.to_path_buf(),
            missing: "a UTF-8 path".into(),
        })?;
        Ok(WidePath(
            text.chars()
                .map(|character| character as libc::wchar_t)
                .chain(std::iter::once(0))
                .collect(),
        ))
    }

    pub(crate) fn as_ptr(&self) -> *const libc::wchar_t {
        self.0.as_ptr()
    }

    pub(crate) fn model(&self) -> ModelInfo {
        ModelInfo {
            path: self.as_ptr(),
            blob: std::ptr::null(),
            blob_size: 0,
        }
    }
}
