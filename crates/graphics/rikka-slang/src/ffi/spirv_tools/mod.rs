use core::ffi::{c_char, c_int, c_void};
use std::sync::OnceLock;

pub(crate) const SPV_SUCCESS: c_int = 0;

pub(crate) const SPV_ENV_UNIVERSAL_1_6: c_int = 24;

#[repr(C)]
pub(crate) struct SpvBinary {
    pub code: *mut u32,
    pub word_count: usize,
}

#[allow(non_snake_case)]
pub(crate) struct SpirvTools {
    pub spvOptimizerCreate: unsafe extern "C" fn(c_int) -> *mut c_void,
    pub spvOptimizerDestroy: unsafe extern "C" fn(*mut c_void),
    pub spvOptimizerRegisterPassesFromFlags:
        unsafe extern "C" fn(*mut c_void, *const *const c_char, usize) -> bool,
    pub spvOptimizerRun: unsafe extern "C" fn(
        *mut c_void,
        *const u32,
        usize,
        *mut *mut SpvBinary,
        *mut c_void,
    ) -> c_int,
    pub spvOptimizerOptionsCreate: unsafe extern "C" fn() -> *mut c_void,
    pub spvOptimizerOptionsDestroy: unsafe extern "C" fn(*mut c_void),
    pub spvOptimizerOptionsSetRunValidator: unsafe extern "C" fn(*mut c_void, bool),
    pub spvBinaryDestroy: unsafe extern "C" fn(*mut SpvBinary),
}

macro_rules! resolve {
    ($library:expr, $($name:ident),* $(,)?) => {
        SpirvTools {
            $($name: *unsafe { $library.get(concat!(stringify!($name), "\0").as_bytes()) }.ok()?,)*
        }
    };
}

macro_rules! resolve_tools {
    ($library:expr) => {
        Some(resolve!(
            $library,
            spvOptimizerCreate,
            spvOptimizerDestroy,
            spvOptimizerRegisterPassesFromFlags,
            spvOptimizerRun,
            spvOptimizerOptionsCreate,
            spvOptimizerOptionsDestroy,
            spvOptimizerOptionsSetRunValidator,
            spvBinaryDestroy,
        ))
    };
}

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix::find;
#[cfg(windows)]
use windows::find;

static TOOLS: OnceLock<Result<SpirvTools, String>> = OnceLock::new();

pub(crate) fn spirv_tools() -> Result<&'static SpirvTools, String> {
    TOOLS.get_or_init(find).as_ref().map_err(Clone::clone)
}

pub(in crate::ffi) fn preload() {
    TOOLS.get_or_init(find);
}
