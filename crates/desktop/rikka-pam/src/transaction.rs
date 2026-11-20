use std::ffi::{CStr, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

use crate::conversation::{Answer, Conversation, converse};
use crate::ffi::{
    self, ACCT_EXPIRED, AUTH_ERR, AUTHINFO_UNAVAIL, CRED_INSUFFICIENT, Handle, MAXTRIES,
    NEW_AUTHTOK_REQD, PERM_DENIED, PamConv, SUCCESS, USER_UNKNOWN,
};
use crate::step::{Outcome, Step};

pub(crate) fn authenticate(
    service: &CStr,
    user: &CStr,
    report: &mut dyn FnMut(Step),
    answers: &Receiver<Answer>,
    cancelled: &AtomicBool,
) -> Outcome {
    let mut conversation = Conversation {
        report,
        answers,
        cancelled,
        serial: 0,
    };
    let conversation = PamConv {
        conv: Some(converse),
        appdata_ptr: (&raw mut conversation).cast(),
    };
    let mut handle = match Handle::start(service, user, &conversation) {
        Ok(handle) => handle,
        Err(code) => return Outcome::Error(ffi::describe(code)),
    };
    let code = handle.authenticate();
    if code != SUCCESS {
        return outcome_for(code, cancelled);
    }
    let code = handle.account();
    if code != SUCCESS && code != NEW_AUTHTOK_REQD {
        return outcome_for(code, cancelled);
    }
    let refreshed = handle.refresh_credentials();
    if refreshed != SUCCESS {
        eprintln!(
            "rikka-pam-helper: failed to refresh credentials: {}",
            ffi::describe(refreshed)
        );
    }
    Outcome::Granted
}

fn outcome_for(code: c_int, cancelled: &AtomicBool) -> Outcome {
    if cancelled.load(Ordering::Acquire) {
        return Outcome::Cancelled;
    }
    match code {
        AUTH_ERR | MAXTRIES | USER_UNKNOWN | CRED_INSUFFICIENT | AUTHINFO_UNAVAIL | PERM_DENIED
        | ACCT_EXPIRED => Outcome::Denied(ffi::describe(code)),
        _ => Outcome::Error(ffi::describe(code)),
    }
}
