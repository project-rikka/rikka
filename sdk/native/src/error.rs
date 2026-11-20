use std::cell::RefCell;
use std::fmt::Display;

use crate::abi;
use crate::ffi;

thread_local! {
    static LAST: RefCell<String> = const { RefCell::new(String::new()) };
}

pub(crate) fn fail(status: ffi::Status, error: impl Display) -> ffi::Status {
    let message = error.to_string();
    LAST.with_borrow_mut(|last| *last = message);
    status
}

pub(crate) fn last() -> ffi::Str {
    LAST.with_borrow(|last| abi::view(last))
}

pub(crate) fn app_status(error: &rikka_app::Error) -> ffi::Status {
    match error {
        rikka_app::Error::NoDisplay | rikka_app::Error::Display(_) => ffi::Status::NoDisplay,
        rikka_app::Error::Renderer(_) => ffi::Status::Gpu,
        rikka_app::Error::Runtime(_) => ffi::Status::System,
        rikka_app::Error::I18n(_) => ffi::Status::Failed,
    }
}
