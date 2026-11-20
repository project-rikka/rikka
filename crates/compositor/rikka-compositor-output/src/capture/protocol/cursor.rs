use rikka_compositor_protocols::ext_image_copy_capture_cursor_session_v1::{
    Error, ExtImageCopyCaptureCursorSessionV1, Request,
};

use super::session;
use crate::capture::host::CaptureHost;
use crate::ids::CursorSessionId;

pub(in crate::capture) fn request<S: CaptureHost>(
    state: &mut S,
    resource: ExtImageCopyCaptureCursorSessionV1,
    request: Request,
) {
    let Request::GetCaptureSession { session: capture } = request else {
        return;
    };
    let Some(id) = resource.key::<CursorSessionId>() else {
        return;
    };
    let Some(cursor) = state.captures().cursors.get_mut(id) else {
        return;
    };
    if std::mem::replace(&mut cursor.session_taken, true) {
        resource.post_error(Error::DuplicateSession);
        return;
    }
    let source = cursor.source;
    session::start(state, capture, source, false, Some(id));
}

pub(in crate::capture) fn destroyed<S: CaptureHost>(
    state: &mut S,
    resource: ExtImageCopyCaptureCursorSessionV1,
) {
    if let Some(id) = resource.key::<CursorSessionId>() {
        state.captures().cursors.remove(id);
    }
}
