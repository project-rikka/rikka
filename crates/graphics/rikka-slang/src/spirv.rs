use core::ffi::c_char;

use crate::error::{Error, Result};
use crate::ffi::{SPV_ENV_UNIVERSAL_1_6, SPV_SUCCESS, SpvBinary, spirv_tools};
use crate::layout::{EntryPoint, Stage};

const FLATTEN_PASSES: [&core::ffi::CStr; 6] = [
    c"--wrap-opkill",
    c"--eliminate-dead-branches",
    c"--merge-return",
    c"--inline-entry-points-exhaustive",
    c"--eliminate-dead-functions",
    c"--eliminate-dead-code-aggressive",
];

pub(crate) fn invariant_entry(mut entry: EntryPoint) -> EntryPoint {
    if entry.stage == Stage::Vertex {
        rikka_spirv::invariant_position(&mut entry.spirv);
    }
    entry
}

pub(crate) fn flatten(mut entry: EntryPoint) -> Result<EntryPoint> {
    let tools = spirv_tools().map_err(Error::Flatten)?;
    let flags = FLATTEN_PASSES.map(|flag| flag.as_ptr());
    unsafe {
        let optimizer = (tools.spvOptimizerCreate)(SPV_ENV_UNIVERSAL_1_6);
        let options = (tools.spvOptimizerOptionsCreate)();
        (tools.spvOptimizerOptionsSetRunValidator)(options, false);
        let registered = (tools.spvOptimizerRegisterPassesFromFlags)(
            optimizer,
            flags.as_ptr().cast::<*const c_char>(),
            flags.len(),
        );
        let mut binary: *mut SpvBinary = core::ptr::null_mut();
        let code = match registered {
            true => (tools.spvOptimizerRun)(
                optimizer,
                entry.spirv.as_ptr(),
                entry.spirv.len(),
                &mut binary,
                options,
            ),
            false => -1,
        };
        (tools.spvOptimizerOptionsDestroy)(options);
        (tools.spvOptimizerDestroy)(optimizer);
        if code != SPV_SUCCESS || binary.is_null() {
            if !binary.is_null() {
                (tools.spvBinaryDestroy)(binary);
            }
            return Err(Error::Flatten(format!(
                "SPIRV-Tools refused `{}` with {code}",
                entry.name
            )));
        }
        entry.spirv = core::slice::from_raw_parts((*binary).code, (*binary).word_count).to_vec();
        (tools.spvBinaryDestroy)(binary);
    }
    Ok(entry)
}
