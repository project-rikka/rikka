use rikka_compositor_protocols::wl_output::{self, WlOutput};
use rikka_compositor_protocols::zxdg_output_manager_v1::{self, ZxdgOutputManagerV1};
use rikka_compositor_protocols::zxdg_output_v1::{self, ZxdgOutputV1};
use rikka_compositor_types::OutputId;

use super::host::OutputHost;

pub(super) fn request<S>(_state: &mut S, _output: WlOutput, request: wl_output::Request) {
    match request {
        wl_output::Request::Release => {}
    }
}

pub(super) fn bound<S: OutputHost>(state: &mut S, output: WlOutput) {
    let Some(id) = output.key::<OutputId>() else {
        return;
    };
    if state.outputs().bind(id, &output) {
        state.output_bound(id, &output);
    }
}

pub(super) fn destroyed<S: OutputHost>(state: &mut S, output: WlOutput) {
    if let Some(id) = output.key::<OutputId>() {
        state.outputs().forget(id, &output);
    }
}

pub(super) fn manager<S: OutputHost>(
    state: &mut S,
    _manager: ZxdgOutputManagerV1,
    request: zxdg_output_manager_v1::Request,
) {
    let zxdg_output_manager_v1::Request::GetXdgOutput { id, output } = request else {
        return;
    };
    let outputs = state.outputs();
    let Some(target) = outputs.output_of(&output) else {
        return;
    };
    id.set_key(target);
    outputs.attach_xdg(target, &id, &output);
}

pub(super) fn xdg_request<S>(_state: &mut S, _xdg: ZxdgOutputV1, request: zxdg_output_v1::Request) {
    match request {
        zxdg_output_v1::Request::Destroy => {}
    }
}

pub(super) fn xdg_destroyed<S: OutputHost>(state: &mut S, xdg: ZxdgOutputV1) {
    if let Some(id) = xdg.key::<OutputId>() {
        state.outputs().forget_xdg(id, &xdg);
    }
}
