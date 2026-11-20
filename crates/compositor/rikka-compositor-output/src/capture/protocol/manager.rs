use rikka_compositor_protocols::ext_image_copy_capture_manager_v1::{
    Error, ExtImageCopyCaptureManagerV1, Options, Request,
};
use rikka_wayland_wire::Resource;

use super::{session, source};
use crate::capture::host::CaptureHost;
use crate::capture::store::Cursor;

pub(in crate::capture) fn request<S: CaptureHost>(
    state: &mut S,
    manager: ExtImageCopyCaptureManagerV1,
    request: Request,
) {
    match request {
        Request::CreateSession {
            session: resource,
            source,
            options,
        } => {
            if Options::from_bits(options.bits()).is_none() {
                manager.object().post_error(
                    Error::InvalidOption.code(),
                    &format!("unknown capture options {:#x}", options.bits()),
                );
                return;
            }
            let source = source::resolve::<S>(&source);
            session::start(
                state,
                resource,
                source,
                options.contains(Options::PAINT_CURSORS),
                None,
            );
        }
        Request::CreatePointerCursorSession {
            session: resource,
            source,
            pointer,
        } => {
            let source = source::resolve::<S>(&source);
            let id = state.captures().cursors.insert(Cursor {
                resource: resource.clone(),
                source,
                pointer,
                session_taken: false,
                placement: None,
            });
            resource.set_key(id);
        }
        Request::Destroy => {}
    }
}
