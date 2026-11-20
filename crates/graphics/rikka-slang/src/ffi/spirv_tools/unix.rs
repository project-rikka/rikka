use core::ffi::{CStr, c_int, c_void};
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use libloading::os::unix::Library;

use super::SpirvTools;
use crate::ffi::functions;

const GLSLANG_MODULE: &str = "libslang-glslang";

fn resolve(library: &Library) -> Option<SpirvTools> {
    resolve_tools!(library)
}

pub(super) fn find() -> Result<SpirvTools, String> {
    let module = load_glslang_module();
    loaded().ok_or_else(|| match module {
        Ok(()) => "no library loaded with libslang exports the SPIRV-Tools optimizer".to_owned(),
        Err(error) => error,
    })
}

fn loaded() -> Option<SpirvTools> {
    loaded_objects().into_iter().find_map(|name| {
        let library =
            unsafe { Library::open(Some(&name), libc::RTLD_LAZY | libc::RTLD_NOLOAD) }.ok()?;
        let tools = resolve(&library)?;
        std::mem::forget(library);
        Some(tools)
    })
}

fn loaded_objects() -> Vec<OsString> {
    unsafe extern "C" fn visit(
        info: *mut libc::dl_phdr_info,
        _size: usize,
        names: *mut c_void,
    ) -> c_int {
        let names = unsafe { &mut *names.cast::<Vec<OsString>>() };
        let name = unsafe { (*info).dlpi_name };
        if !name.is_null() {
            let name = unsafe { CStr::from_ptr(name) };
            if !name.is_empty() {
                names.push(OsStr::from_bytes(name.to_bytes()).to_owned());
            }
        }
        0
    }
    let mut names: Vec<OsString> = Vec::new();
    unsafe {
        libc::dl_iterate_phdr(Some(visit), (&raw mut names).cast());
    }
    names
}

fn load_glslang_module() -> Result<(), String> {
    let mut info = libc::Dl_info {
        dli_fname: core::ptr::null(),
        dli_fbase: core::ptr::null_mut(),
        dli_sname: core::ptr::null(),
        dli_saddr: core::ptr::null_mut(),
    };
    let anchor = functions().slang_createBlob as *const c_void;
    if unsafe { libc::dladdr(anchor, &mut info) } == 0 || info.dli_fname.is_null() {
        return Err("libslang's location is unknown".to_owned());
    }
    let path = unsafe { CStr::from_ptr(info.dli_fname) }
        .to_str()
        .map_err(|_| "libslang's path is not UTF-8".to_owned())?;
    let directory = Path::new(path)
        .parent()
        .ok_or_else(|| format!("{path} has no directory"))?;
    let module = std::fs::read_dir(directory)
        .map_err(|error| format!("failed to list {}: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|candidate| {
            candidate
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(GLSLANG_MODULE) && name.ends_with(".so"))
        })
        .ok_or_else(|| format!("{} has no {GLSLANG_MODULE}", directory.display()))?;
    let library = unsafe { Library::open(Some(&module), libc::RTLD_LAZY | libc::RTLD_LOCAL) }
        .map_err(|error| format!("failed to load {}: {error}", module.display()))?;
    std::mem::forget(library);
    Ok(())
}
