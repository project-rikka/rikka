use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::path::Path;

use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    ILCreateFromPathW, ILFree, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, SHOpenFolderAndSelectItems,
    ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

const NAME: &str = "rikka-open-shell";

pub(crate) fn in_apartment<T: Send + 'static>(
    work: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    std::thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || {
            let _apartment = Apartment::enter()?;
            work()
        })?
        .join()
        .map_err(|_| io::Error::other("shell thread panicked"))?
}

pub(crate) fn execute(file: &OsStr) -> io::Result<()> {
    let file = wide(file);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC,
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(io::Error::from)
}

pub(crate) fn select(path: &Path) -> io::Result<()> {
    let file = wide(path.as_os_str());
    let item = unsafe { ILCreateFromPathW(PCWSTR(file.as_ptr())) };
    if item.is_null() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    let shown = unsafe { SHOpenFolderAndSelectItems(item, None, 0) };
    unsafe { ILFree(Some(item.cast_const())) };
    shown.map_err(io::Error::from)
}

fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

struct Apartment;

impl Apartment {
    fn enter() -> io::Result<Apartment> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
            .ok()
            .map_err(io::Error::from)?;
        Ok(Apartment)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
