use std::ffi::{CStr, CString, c_char, c_int, c_void};

use crate::error::{Error, Result};

pub(crate) const SUCCESS: c_int = 0;
pub(crate) const BUF_ERR: c_int = 5;
pub(crate) const PERM_DENIED: c_int = 6;
pub(crate) const AUTH_ERR: c_int = 7;
pub(crate) const CRED_INSUFFICIENT: c_int = 8;
pub(crate) const AUTHINFO_UNAVAIL: c_int = 9;
pub(crate) const USER_UNKNOWN: c_int = 10;
pub(crate) const MAXTRIES: c_int = 11;
pub(crate) const NEW_AUTHTOK_REQD: c_int = 12;
pub(crate) const ACCT_EXPIRED: c_int = 13;
pub(crate) const CONV_ERR: c_int = 19;

pub(crate) const PROMPT_ECHO_OFF: c_int = 1;
pub(crate) const PROMPT_ECHO_ON: c_int = 2;
pub(crate) const ERROR_MSG: c_int = 3;
pub(crate) const TEXT_INFO: c_int = 4;

pub(crate) fn c_string(text: &str) -> Result<CString> {
    CString::new(text).map_err(|_| Error::Nul(text.to_owned()))
}

const REFRESH_CRED: c_int = 0x0010;

#[repr(C)]
pub(crate) struct PamMessage {
    pub(crate) msg_style: c_int,
    pub(crate) msg: *const c_char,
}

#[repr(C)]
pub(crate) struct PamResponse {
    pub(crate) resp: *mut c_char,
    pub(crate) resp_retcode: c_int,
}

pub(crate) type Converse = unsafe extern "C" fn(
    count: c_int,
    messages: *mut *const PamMessage,
    responses: *mut *mut PamResponse,
    conversation: *mut c_void,
) -> c_int;

#[repr(C)]
pub(crate) struct PamConv {
    pub(crate) conv: Option<Converse>,
    pub(crate) appdata_ptr: *mut c_void,
}

#[repr(C)]
struct PamHandle {
    _opaque: [u8; 0],
}

#[link(name = "pam")]
unsafe extern "C" {
    fn pam_start(
        service: *const c_char,
        user: *const c_char,
        conversation: *const PamConv,
        handle: *mut *mut PamHandle,
    ) -> c_int;
    fn pam_end(handle: *mut PamHandle, status: c_int) -> c_int;
    fn pam_authenticate(handle: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_acct_mgmt(handle: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_setcred(handle: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_strerror(handle: *mut PamHandle, code: c_int) -> *const c_char;
}

pub(crate) struct Handle {
    raw: *mut PamHandle,
    status: c_int,
}

impl Handle {
    pub(crate) fn start(
        service: &CStr,
        user: &CStr,
        conversation: &PamConv,
    ) -> Result<Handle, c_int> {
        let mut raw = std::ptr::null_mut();
        let code = unsafe { pam_start(service.as_ptr(), user.as_ptr(), conversation, &mut raw) };
        match code == SUCCESS && !raw.is_null() {
            true => Ok(Handle { raw, status: code }),
            false => Err(code),
        }
    }

    pub(crate) fn authenticate(&mut self) -> c_int {
        self.record_status(unsafe { pam_authenticate(self.raw, 0) })
    }

    pub(crate) fn account(&mut self) -> c_int {
        self.record_status(unsafe { pam_acct_mgmt(self.raw, 0) })
    }

    pub(crate) fn refresh_credentials(&mut self) -> c_int {
        unsafe { pam_setcred(self.raw, REFRESH_CRED) }
    }

    fn record_status(&mut self, code: c_int) -> c_int {
        self.status = code;
        code
    }
}

pub(crate) fn describe(code: c_int) -> String {
    let text = unsafe { pam_strerror(std::ptr::null_mut(), code) };
    match text.is_null() {
        true => format!("PAM error {code}"),
        false => unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned(),
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { pam_end(self.raw, self.status) };
    }
}
