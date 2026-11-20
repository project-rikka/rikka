use crate::abi::{AnyThread, Closure};
use crate::ffi;
use crate::handlers::{Callback, action};
use crate::input;
use crate::window::{Window, WindowBuilder};

pub(crate) fn window_builder_new(view: Callback<ffi::ViewFn>) -> WindowBuilder {
    WindowBuilder::new(view)
}

pub(crate) fn window_builder_set_title(builder: &mut WindowBuilder, title: &str) {
    builder.set_title(title);
}

pub(crate) fn window_builder_set_size(builder: &mut WindowBuilder, width: f32, height: f32) {
    builder.set_size(width, height);
}

pub(crate) fn window_builder_set_min_size(builder: &mut WindowBuilder, width: f32, height: f32) {
    builder.set_min_size(width, height);
}

pub(crate) fn window_builder_set_resizable(builder: &mut WindowBuilder, resizable: bool) {
    builder.set_resizable(resizable);
}

pub(crate) fn window_builder_set_maximized(builder: &mut WindowBuilder, maximized: bool) {
    builder.set_maximized(maximized);
}

pub(crate) fn window_builder_set_parent(builder: &mut WindowBuilder, parent: &Window) {
    builder.set_parent(parent.reference());
}

pub(crate) fn window_builder_set_modal(builder: &mut WindowBuilder, modal: bool) {
    builder.set_modal(modal);
}

pub(crate) fn window_builder_on_start(
    builder: &mut WindowBuilder,
    callback: Callback<ffi::ActionFn>,
) {
    builder.on_start(callback);
}

pub(crate) fn window_builder_on_close_request(
    builder: &mut WindowBuilder,
    callback: Callback<ffi::ActionFn>,
) {
    builder.on_close_request(callback);
}

pub(crate) fn window_builder_on_focus_change(
    builder: &mut WindowBuilder,
    callback: Callback<ffi::BoolFn>,
) {
    builder.on_focus_change(callback);
}

pub(crate) fn window_builder_on_shape(
    builder: &mut WindowBuilder,
    callback: Callback<ffi::ShapeFn>,
) {
    builder.on_shape(callback);
}

pub(crate) fn window_builder_on_launch(
    builder: &mut WindowBuilder,
    callback: Callback<ffi::LaunchFn>,
) {
    builder.on_launch(callback);
}

pub(crate) fn window_builder_add_shortcut(
    builder: &mut WindowBuilder,
    key: ffi::KeyCode,
    modifiers: ffi::Modifiers,
    callback: Callback<ffi::ActionFn>,
) {
    builder.add_shortcut(
        input::key(key),
        input::modifiers(modifiers),
        action(callback),
    );
}

pub(crate) fn window_builder_add_command(
    builder: &mut WindowBuilder,
    command: ffi::Command,
    callback: Callback<ffi::ActionFn>,
) {
    builder.add_command(input::command(command), action(callback));
}

pub(crate) fn window_invalidate(window: &Window) {
    window.invalidate();
}

pub(crate) fn window_post(
    window: &Window,
    callback: Closure<ffi::ActionFn, AnyThread>,
) -> ffi::Status {
    window.post(callback)
}

pub(crate) fn window_is_open(window: &Window) -> bool {
    window.is_open()
}

pub(crate) fn window_set_title(window: &Window, title: &str) {
    window.set_title(title);
}
