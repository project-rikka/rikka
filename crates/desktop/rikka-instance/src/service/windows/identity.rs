use std::io;
use std::os::windows::io::{AsRawHandle as _, BorrowedHandle, FromRawHandle as _, OwnedHandle};

use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_KERNEL_OBJECT,
};
use windows::Win32::Security::{
    GetTokenInformation, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    SECURITY_ATTRIBUTES, TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_USER, TokenSessionId,
    TokenUser,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{HSTRING, PWSTR};

pub(super) struct Identity {
    pub(super) sid: String,
    pub(super) session: u32,
}

impl Identity {
    pub(super) fn current() -> io::Result<Identity> {
        let mut raw = HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) }?;
        let token = unsafe { OwnedHandle::from_raw_handle(raw.0) };
        let user = information(&token, TokenUser)?;
        let user = unsafe { &*user.as_ptr().cast::<TOKEN_USER>() };
        let sid = sid_text(user.User.Sid)?;
        let session = information(&token, TokenSessionId)?;
        Ok(Identity {
            sid,
            session: session.first().map_or(0, |word| *word as u32),
        })
    }
}

pub(super) struct Descriptor(PSECURITY_DESCRIPTOR);

impl Descriptor {
    pub(super) fn owner_only(sid: &str) -> io::Result<Descriptor> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let text = HSTRING::from(format!("O:{sid}D:P(A;;GA;;;{sid})"));
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &text,
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }?;
        Ok(Descriptor(descriptor))
    }

    pub(super) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0.0,
            bInheritHandle: false.into(),
        }
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe { LocalFree(Some(HLOCAL(self.0.0))) };
    }
}

unsafe impl Send for Descriptor {}

pub(super) fn owner_of(handle: BorrowedHandle<'_>) -> io::Result<String> {
    let mut owner = PSID::default();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let status = unsafe {
        GetSecurityInfo(
            HANDLE(handle.as_raw_handle()),
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            None,
            None,
            Some(&mut descriptor),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status.0 as i32));
    }
    let text = sid_text(owner);
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    text
}

fn sid_text(sid: PSID) -> io::Result<String> {
    let mut text = PWSTR::null();
    unsafe { ConvertSidToStringSidW(sid, &mut text) }?;
    let converted = unsafe { text.to_string() }.map_err(io::Error::other);
    unsafe { LocalFree(Some(HLOCAL(text.0.cast()))) };
    converted
}

fn information(token: &OwnedHandle, class: TOKEN_INFORMATION_CLASS) -> io::Result<Vec<u64>> {
    let token = HANDLE(token.as_raw_handle());
    let mut length = 0u32;
    let _ = unsafe { GetTokenInformation(token, class, None, 0, &mut length) };
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0u64; (length as usize).div_ceil(size_of::<u64>())];
    unsafe {
        GetTokenInformation(
            token,
            class,
            Some(buffer.as_mut_ptr().cast()),
            length,
            &mut length,
        )
    }?;
    Ok(buffer)
}
