use std::sync::Arc;

use rikka_app::embedded::Background;
use rikka_ui::Event;

use crate::embedding::{
    EmbeddedSurface, EmbeddedSurfaceBuilder, Embedding, Handles, Submit, VulkanDevice,
};
use crate::ffi;
use crate::handlers::Callback;
use crate::input::{self, button, caret, key_press, key_release, point, typed, wheel};
use crate::window::Window;

#[allow(clippy::too_many_arguments)]
pub(crate) fn vulkan_device_new(
    get_instance_proc_addr: u64,
    instance: u64,
    api_version: u32,
    physical_device: u64,
    device: u64,
    queue: u64,
    queue_family: u32,
    submit: Submit,
) -> VulkanDevice {
    VulkanDevice::new(
        Handles {
            get_instance_proc_addr,
            instance,
            api_version,
            physical_device,
            device,
            queue,
            queue_family,
        },
        submit,
    )
}

pub(crate) fn vulkan_device_add_instance_extension(device: &mut VulkanDevice, name: &str) {
    device.add_instance_extension(name);
}

pub(crate) fn vulkan_device_add_device_extension(device: &mut VulkanDevice, name: &str) {
    device.add_device_extension(name);
}

pub(crate) fn vulkan_device_set_features(device: &mut VulkanDevice, features: u64) {
    device.set_features(features);
}

pub(crate) fn embedding_new(device: VulkanDevice) -> Embedding {
    Embedding::new(device)
}

pub(crate) fn embedding_error_message(embedding: &Embedding) -> String {
    embedding.error_message()
}

pub(crate) fn embedding_open(
    embedding: &Embedding,
    surface: EmbeddedSurfaceBuilder,
) -> EmbeddedSurface {
    embedding.open(surface)
}

pub(crate) fn embedding_set_theme(embedding: &Embedding, slug: &str) {
    embedding.set_theme(slug);
}

pub(crate) fn embedding_set_background(embedding: &Embedding, background: ffi::ThemeBackground) {
    embedding.set_background(match background {
        ffi::ThemeBackground::Desktop => None,
        ffi::ThemeBackground::Solid => Some(Background::Solid),
        ffi::ThemeBackground::Glass => Some(Background::Glass),
        ffi::ThemeBackground::Liquid => Some(Background::Liquid),
    });
}

pub(crate) fn embedded_surface_builder_new(view: Callback<ffi::ViewFn>) -> EmbeddedSurfaceBuilder {
    EmbeddedSurfaceBuilder::new(view)
}

pub(crate) fn embedded_surface_builder_on_start(
    builder: &mut EmbeddedSurfaceBuilder,
    callback: Callback<ffi::ActionFn>,
) {
    builder.on_start(callback);
}

pub(crate) fn embedded_surface_builder_on_close_request(
    builder: &mut EmbeddedSurfaceBuilder,
    callback: Callback<ffi::ActionFn>,
) {
    builder.on_close_request(callback);
}

pub(crate) fn embedded_surface_builder_on_left(
    builder: &mut EmbeddedSurfaceBuilder,
    callback: Callback<ffi::ActionFn>,
) {
    builder.on_left(callback);
}

pub(crate) fn embedded_surface_leave(surface: &EmbeddedSurface) {
    surface.leave();
}

pub(crate) fn embedded_surface_rejoin(surface: &EmbeddedSurface) {
    surface.rejoin();
}

pub(crate) fn embedded_surface_window(surface: &EmbeddedSurface) -> Arc<Window> {
    surface.window()
}

pub(crate) fn embedded_surface_resize(
    surface: &EmbeddedSurface,
    width: f32,
    height: f32,
    scale: f32,
) {
    surface.resize(width, height, scale);
}

pub(crate) fn embedded_surface_render(
    surface: &EmbeddedSurface,
    target: ffi::VulkanImage,
) -> ffi::Status {
    surface.render(target)
}

pub(crate) fn embedded_surface_pointer_move(surface: &EmbeddedSurface, x: f32, y: f32) -> bool {
    surface.handle(Event::PointerMoved(point(x, y)))
}

pub(crate) fn embedded_surface_pointer_press(
    surface: &EmbeddedSurface,
    x: f32,
    y: f32,
    pressed: ffi::PointerButton,
    modifiers: ffi::Modifiers,
) -> bool {
    surface.handle(Event::PointerPressed {
        position: point(x, y),
        button: button(pressed),
        modifiers: input::modifiers(modifiers),
    })
}

pub(crate) fn embedded_surface_pointer_release(
    surface: &EmbeddedSurface,
    x: f32,
    y: f32,
    released: ffi::PointerButton,
    modifiers: ffi::Modifiers,
) -> bool {
    surface.handle(Event::PointerReleased {
        position: point(x, y),
        button: button(released),
        modifiers: input::modifiers(modifiers),
    })
}

pub(crate) fn embedded_surface_pointer_leave(surface: &EmbeddedSurface) {
    surface.handle(Event::PointerLeft);
}

pub(crate) fn embedded_surface_scroll(
    surface: &EmbeddedSurface,
    x: f32,
    y: f32,
    steps_x: f32,
    steps_y: f32,
    modifiers: ffi::Modifiers,
) -> bool {
    surface.handle(wheel(
        point(x, y),
        steps_x,
        steps_y,
        input::modifiers(modifiers),
    ))
}

pub(crate) fn embedded_surface_key_press(
    surface: &EmbeddedSurface,
    key: ffi::KeyCode,
    modifiers: ffi::Modifiers,
) -> bool {
    surface.handle(key_press(input::key(key), input::modifiers(modifiers)))
}

pub(crate) fn embedded_surface_key_release(
    surface: &EmbeddedSurface,
    key: ffi::KeyCode,
    modifiers: ffi::Modifiers,
) -> bool {
    surface.handle(key_release(input::key(key), input::modifiers(modifiers)))
}

pub(crate) fn embedded_surface_type_text(surface: &EmbeddedSurface, text: &str) -> bool {
    typed(text).fold(false, |used, key| {
        let pressed = surface.handle(key_press(key, rikka_ui::Modifiers::NONE));
        let released = surface.handle(key_release(key, rikka_ui::Modifiers::NONE));
        used | pressed | released
    })
}

pub(crate) fn embedded_surface_compose_text(
    surface: &EmbeddedSurface,
    text: &str,
    caret_start: usize,
    caret_end: usize,
) -> bool {
    surface.handle(Event::Preedit {
        cursor: caret(text, caret_start, caret_end),
        text: text.to_owned(),
    })
}

pub(crate) fn embedded_surface_commit_text(surface: &EmbeddedSurface, text: &str) -> bool {
    surface.handle(Event::Commit(text.to_owned()))
}

pub(crate) fn embedded_surface_cursor(surface: &EmbeddedSurface) -> ffi::CursorShape {
    surface.cursor()
}

pub(crate) fn embedded_surface_caret(surface: &EmbeddedSurface) -> ffi::Rect {
    surface.caret()
}
