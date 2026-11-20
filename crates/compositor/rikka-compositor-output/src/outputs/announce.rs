use rikka_compositor_protocols::wl_output::{self, WlOutput};
use rikka_compositor_protocols::zxdg_output_v1::ZxdgOutputV1;

use super::info::OutputInfo;

const XDG_DONE_DEPRECATED_SINCE: u32 = 3;

pub(super) fn wl_burst(output: &WlOutput, name: &str, info: &OutputInfo) {
    geometry(output, info);
    mode(output, info);
    output.name(name);
    output.description(&info.description);
    output.scale(info.integer_scale());
    output.done();
}

pub(super) fn xdg_burst(xdg: &ZxdgOutputV1, output: &WlOutput, name: &str, info: &OutputInfo) {
    logical_position(xdg, info);
    logical_size(xdg, info);
    xdg.name(name);
    xdg.description(&info.description);
    xdg_done(xdg);
    output.done();
}

pub(super) fn wl_changes(output: &WlOutput, old: &OutputInfo, new: &OutputInfo) -> bool {
    let geometry_changed = old.position != new.position
        || old.physical_size != new.physical_size
        || old.subpixel != new.subpixel
        || old.make != new.make
        || old.model != new.model
        || old.transform != new.transform;
    if geometry_changed {
        geometry(output, new);
    }
    let mode_changed = old.mode != new.mode;
    if mode_changed {
        mode(output, new);
    }
    let scale_changed = old.integer_scale() != new.integer_scale();
    if scale_changed {
        output.scale(new.integer_scale());
    }
    let description_changed = old.description != new.description;
    if description_changed {
        output.description(&new.description);
    }
    geometry_changed || mode_changed || scale_changed || description_changed
}

pub(super) fn xdg_changes(xdg: &ZxdgOutputV1, old: &OutputInfo, new: &OutputInfo) -> bool {
    let mut changed = false;
    if old.position != new.position {
        logical_position(xdg, new);
        changed = true;
    }
    if old.logical_size() != new.logical_size() {
        logical_size(xdg, new);
        changed = true;
    }
    if old.description != new.description && xdg.version() >= XDG_DONE_DEPRECATED_SINCE {
        xdg.description(&new.description);
        changed = true;
    }
    if changed {
        xdg_done(xdg);
    }
    changed
}
fn geometry(output: &WlOutput, info: &OutputInfo) {
    let transform = wl_output::Transform::try_from(info.transform.to_wayland())
        .unwrap_or(wl_output::Transform::Normal);
    output.geometry(
        info.position.x,
        info.position.y,
        info.physical_size.width,
        info.physical_size.height,
        info.subpixel,
        &info.make,
        &info.model,
        transform,
    );
}

fn mode(output: &WlOutput, info: &OutputInfo) {
    output.mode(
        wl_output::Mode::CURRENT,
        info.mode.size.w,
        info.mode.size.h,
        info.mode.refresh_millihertz,
    );
}

fn logical_position(xdg: &ZxdgOutputV1, info: &OutputInfo) {
    xdg.logical_position(info.position.x, info.position.y);
}

fn logical_size(xdg: &ZxdgOutputV1, info: &OutputInfo) {
    let size = info.logical_size();
    xdg.logical_size(size.w, size.h);
}

fn xdg_done(xdg: &ZxdgOutputV1) {
    if xdg.version() < XDG_DONE_DEPRECATED_SINCE {
        xdg.done();
    }
}
