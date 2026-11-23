use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub struct Shipped {
    pub variable: &'static str,
    pub installed: &'static str,
}

impl Shipped {
    #[must_use]
    pub fn find(&self, file: &str) -> Option<PathBuf> {
        self.folders()
            .map(|folder| folder.join(file))
            .find(|path| path.is_file())
    }

    #[must_use]
    pub fn folder(&self) -> Option<PathBuf> {
        self.folders().find(|folder| folder.is_dir())
    }

    fn folders(&self) -> impl Iterator<Item = PathBuf> {
        let named = env::var_os(self.variable)
            .map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
            .unwrap_or_default();
        let beside = [env::current_exe().ok(), loaded_object()]
            .into_iter()
            .flatten()
            .filter_map(|object| Some(install_root(&object)?.join(self.installed)));
        named.into_iter().chain(beside)
    }
}

#[cfg(unix)]
fn install_root(object: &Path) -> Option<&Path> {
    object.parent()?.parent()
}

#[cfg(windows)]
fn install_root(object: &Path) -> Option<&Path> {
    object.parent()
}

#[cfg(unix)]
fn loaded_object() -> Option<PathBuf> {
    use std::ffi::{CStr, OsStr, c_void};
    use std::os::unix::ffi::OsStrExt as _;
    use std::path::Path;

    let mut info = libc::Dl_info {
        dli_fname: std::ptr::null(),
        dli_fbase: std::ptr::null_mut(),
        dli_sname: std::ptr::null(),
        dli_saddr: std::ptr::null_mut(),
    };
    let anchor = loaded_object as *const c_void;
    if unsafe { libc::dladdr(anchor, &mut info) } == 0 || info.dli_fname.is_null() {
        return None;
    }
    let name = unsafe { CStr::from_ptr(info.dli_fname) };
    let path = Path::new(OsStr::from_bytes(name.to_bytes()));
    path.is_absolute().then(|| path.to_owned())
}

#[cfg(windows)]
fn loaded_object() -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt as _;

    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        GetModuleFileNameW, GetModuleHandleExW,
    };
    use windows::core::PCWSTR;

    const LONGEST_PATH: usize = 32_768;

    let mut module = HMODULE::default();
    let anchor = PCWSTR(loaded_object as *const u16);
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            anchor,
            &raw mut module,
        )
    }
    .ok()?;
    let mut name = vec![0_u16; LONGEST_PATH];
    let length = unsafe { GetModuleFileNameW(Some(module), &mut name) } as usize;
    if length == 0 || length >= name.len() {
        return None;
    }
    name.truncate(length);
    Some(PathBuf::from(OsString::from_wide(&name)))
}
