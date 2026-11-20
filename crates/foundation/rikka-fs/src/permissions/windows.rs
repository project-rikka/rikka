use std::io;
use std::path::Path;

use windows::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, HLOCAL, LocalFree, WIN32_ERROR,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW, SDDL_REVISION_1,
    SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    OBJECT_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    SE_DACL_PROTECTED, UNPROTECTED_DACL_SECURITY_INFORMATION,
};
use windows::core::{BOOL, HSTRING, w};

use crate::replace::Mode;

pub(crate) fn apply(staging: &Path, mode: Mode, target: &Path) -> io::Result<()> {
    let descriptor = match mode {
        Mode::Fixed(_) => owner_only()?,
        Mode::Preserve { .. } => match of_file(target)? {
            Some(descriptor) => descriptor,
            None => return Ok(()),
        },
    };
    descriptor.set_on(staging)
}

struct Descriptor(PSECURITY_DESCRIPTOR);

fn owner_only() -> io::Result<Descriptor> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:P(A;;FA;;;OW)(A;;FA;;;SY)"),
            SDDL_REVISION_1,
            &raw mut descriptor,
            None,
        )
    }?;
    Ok(Descriptor(descriptor))
}

fn of_file(path: &Path) -> io::Result<Option<Descriptor>> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let status = unsafe {
        GetNamedSecurityInfoW(
            &HSTRING::from(path),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            &raw mut descriptor,
        )
    };
    match status {
        ERROR_SUCCESS => Ok(Some(Descriptor(descriptor))),
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Ok(None),
        status => Err(win32_error(status)),
    }
}

impl Descriptor {
    fn set_on(&self, path: &Path) -> io::Result<()> {
        let mut present = BOOL::default();
        let mut defaulted = BOOL::default();
        let mut dacl: *mut ACL = std::ptr::null_mut();
        unsafe {
            GetSecurityDescriptorDacl(self.0, &raw mut present, &raw mut dacl, &raw mut defaulted)
        }?;
        if !present.as_bool() {
            return Ok(());
        }
        let status = unsafe {
            SetNamedSecurityInfoW(
                &HSTRING::from(path),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | self.inheritance()?,
                None,
                None,
                Some(dacl.cast_const()),
                None,
            )
        };
        match status {
            ERROR_SUCCESS => Ok(()),
            status => Err(win32_error(status)),
        }
    }

    fn inheritance(&self) -> io::Result<OBJECT_SECURITY_INFORMATION> {
        let mut control = 0_u16;
        let mut revision = 0_u32;
        unsafe { GetSecurityDescriptorControl(self.0, &raw mut control, &raw mut revision) }?;
        Ok(match control & SE_DACL_PROTECTED.0 {
            0 => UNPROTECTED_DACL_SECURITY_INFORMATION,
            _ => PROTECTED_DACL_SECURITY_INFORMATION,
        })
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe { LocalFree(Some(HLOCAL(self.0.0))) };
    }
}

fn win32_error(status: WIN32_ERROR) -> io::Error {
    io::Error::from_raw_os_error(status.0.cast_signed())
}
