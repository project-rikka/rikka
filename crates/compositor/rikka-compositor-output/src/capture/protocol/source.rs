use rikka_compositor_protocols::ext_foreign_toplevel_image_capture_source_manager_v1::{
    self as toplevel_manager, ExtForeignToplevelImageCaptureSourceManagerV1,
};
use rikka_compositor_protocols::ext_image_capture_source_v1::{self, ExtImageCaptureSourceV1};
use rikka_compositor_protocols::ext_output_image_capture_source_manager_v1::{
    self as output_manager, ExtOutputImageCaptureSourceManagerV1,
};
use rikka_compositor_types::OutputId;

use crate::capture::host::CaptureHost;
use crate::capture::types::CaptureSource;

pub(in crate::capture) fn output_manager<S: CaptureHost>(
    state: &mut S,
    _manager: ExtOutputImageCaptureSourceManagerV1,
    request: output_manager::Request,
) {
    let output_manager::Request::CreateSource { source, output } = request else {
        return;
    };
    if let Some(output) = state.outputs().output_of(&output) {
        source.set_key(output);
    }
}

pub(in crate::capture) fn toplevel_manager<S: CaptureHost>(
    state: &mut S,
    _manager: ExtForeignToplevelImageCaptureSourceManagerV1,
    request: toplevel_manager::Request,
) {
    let toplevel_manager::Request::CreateSource {
        source,
        toplevel_handle,
    } = request
    else {
        return;
    };
    if let Some(toplevel) = state.toplevel_source(&toplevel_handle) {
        source.set_key(toplevel);
    }
}

pub(in crate::capture) fn request<S>(
    _state: &mut S,
    _source: ExtImageCaptureSourceV1,
    request: ext_image_capture_source_v1::Request,
) {
    match request {
        ext_image_capture_source_v1::Request::Destroy => {}
    }
}

pub(super) fn resolve<S: CaptureHost>(
    source: &ExtImageCaptureSourceV1,
) -> Option<CaptureSource<S::Toplevel>> {
    source
        .key::<OutputId>()
        .map(CaptureSource::Output)
        .or_else(|| source.key::<S::Toplevel>().map(CaptureSource::Toplevel))
}
