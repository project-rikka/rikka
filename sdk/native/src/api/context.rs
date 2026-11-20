use std::sync::Arc;
use std::time::Duration;

use rikka_dialog::ChooserResponse;
use rikka_ui::{WindowRequest, after_keyed, cancel, clipboard, every_keyed, hash_of};

use crate::abi::{AnyThread, Closure, violation};
use crate::api::chooser::FileChooser;
use crate::context::Context;
use crate::error;
use crate::ffi;
use crate::handlers::{Callback, action};
use crate::window::{Model, Window, WindowBuilder};

fn timer(key: &str) -> u64 {
    hash_of(("rikka-sdk-timer", key))
}

pub(crate) fn context_window(context: &mut Context) -> &Window {
    context.window()
}

pub(crate) fn context_skip_view(_: &mut Context) {
    rikka_ui::skip_view();
}

pub(crate) fn context_request(_: &mut Context, request: ffi::WindowRequest) {
    rikka_ui::request_window(match request {
        ffi::WindowRequest::Close => WindowRequest::Close,
        ffi::WindowRequest::AskToClose => WindowRequest::AskToClose,
        ffi::WindowRequest::Minimize => WindowRequest::Minimize,
        ffi::WindowRequest::ToggleMaximize => WindowRequest::ToggleMaximize,
        ffi::WindowRequest::EnterFullscreen => WindowRequest::Fullscreen(true),
        ffi::WindowRequest::ExitFullscreen => WindowRequest::Fullscreen(false),
        ffi::WindowRequest::ToggleFullscreen => WindowRequest::ToggleFullscreen,
    });
}

pub(crate) fn context_open_window(_: &mut Context, window: WindowBuilder) {
    rikka_app::open_window(window.into_window());
}

pub(crate) fn context_after(
    _: &mut Context,
    key: &str,
    milliseconds: u32,
    callback: Callback<ffi::ActionFn>,
) {
    after_keyed(
        timer(key),
        Duration::from_millis(u64::from(milliseconds)),
        action(callback),
    );
}

pub(crate) fn context_every(
    _: &mut Context,
    key: &str,
    milliseconds: u32,
    callback: Callback<ffi::ActionFn>,
) {
    if milliseconds == 0 {
        violation("a repeating timer needs a period above 0");
    }
    every_keyed(
        timer(key),
        Duration::from_millis(u64::from(milliseconds)),
        action(callback),
    );
}

pub(crate) fn context_cancel(_: &mut Context, key: &str) {
    cancel(timer(key));
}

pub(crate) fn context_run_in_background(
    _: &mut Context,
    task: Closure<(ffi::WorkFn, ffi::ActionFn), AnyThread>,
) {
    let task = Arc::new(task);
    let worker = Arc::clone(&task);
    rikka_ui::spawn_blocking(
        move || worker.call_work(),
        move |model: &mut Model, (): ()| task.call_done(&mut model.context()),
    );
}

pub(crate) fn context_set_clipboard_text(_: &mut Context, text: &str) {
    clipboard::set_text(text);
}

pub(crate) fn context_read_clipboard_text(_: &mut Context, callback: Callback<ffi::TextFn>) {
    clipboard::read_text(move |model: &mut Model, text: Option<String>| {
        callback.call(
            &mut model.context(),
            text.is_some(),
            text.as_deref().unwrap_or_default(),
        );
    });
}

pub(crate) fn context_choose_files(
    _: &mut Context,
    chooser: FileChooser,
    callback: Callback<ffi::FilesFn>,
) {
    let dialog = chooser.into_chooser().parent(rikka_ui::window_identifier());
    rikka_ui::spawn_blocking(
        move || dialog.run(),
        move |model: &mut Model, chosen: rikka_dialog::Result<ChooserResponse>| {
            let mut context = model.context();
            match chosen {
                Ok(ChooserResponse::Files(paths)) => {
                    let paths: Vec<String> = paths
                        .iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect();
                    let borrowed: Vec<&str> = paths.iter().map(String::as_str).collect();
                    callback.call(&mut context, ffi::Status::Ok, &borrowed);
                }
                Ok(ChooserResponse::Cancelled) => {
                    callback.call(&mut context, ffi::Status::Cancelled, &[]);
                }
                Err(failure) => {
                    let status = error::fail(ffi::Status::Failed, failure);
                    callback.call(&mut context, status, &[]);
                }
            }
        },
    );
}

pub(crate) fn context_alert(context: &mut Context, title: &str, message: &str) {
    context.dialogs().alert(title, message);
}

pub(crate) fn context_confirm(
    context: &mut Context,
    title: &str,
    message: &str,
    accept_label: &str,
    callback: Callback<ffi::BoolFn>,
) {
    if accept_label.is_empty() {
        violation("the accept label is empty");
    }
    context
        .dialogs()
        .confirm(title, message, accept_label, callback);
}
