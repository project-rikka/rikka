use std::cell::RefCell;

use crate::window::{IntoWindows, Opening};

thread_local! {
    static REQUESTED_WINDOWS: RefCell<Option<Vec<Opening>>> = const { RefCell::new(None) };
}

pub fn open_window(window: impl IntoWindows) {
    REQUESTED_WINDOWS.with_borrow_mut(|requested| match requested {
        Some(requested) => requested.extend(window.into_windows()),
        None => tracing::warn!("window requested outside application thread"),
    });
}

pub(crate) fn accept_window_requests() {
    REQUESTED_WINDOWS.with_borrow_mut(|requested| {
        requested.get_or_insert_default();
    });
}

pub(crate) fn take_requested_windows() -> Vec<Opening> {
    REQUESTED_WINDOWS
        .with_borrow_mut(|requested| requested.as_mut().map(std::mem::take).unwrap_or_default())
}
