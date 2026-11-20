use rikka_compositor_geometry::Region;
use rikka_compositor_protocols::ext_image_copy_capture_frame_v1::FailureReason;
use rikka_compositor_protocols::ext_image_copy_capture_session_v1::{
    Error, ExtImageCopyCaptureSessionV1, Request,
};

use crate::capture::constraints;
use crate::capture::host::CaptureHost;
use crate::capture::store::{Frame, Phase, Session};
use crate::capture::types::{CaptureSource, SessionInfo};
use crate::ids::{CaptureSessionId, CursorSessionId};

pub(super) fn start<S: CaptureHost>(
    state: &mut S,
    resource: ExtImageCopyCaptureSessionV1,
    source: Option<CaptureSource<S::Toplevel>>,
    paint_cursors: bool,
    cursor: Option<CursorSessionId>,
) {
    let Some(source) = source else {
        resource.stopped();
        return;
    };
    let info = SessionInfo {
        source,
        paint_cursors,
        cursor,
    };
    let id = state.captures().sessions.insert(Session {
        resource: resource.clone(),
        info,
        constraints: None,
        frame: None,
        delivered: false,
        stopped: false,
    });
    resource.set_key(id);
    let Some(initial) = state.capture_session_started(id, info) else {
        state.captures().stop(id);
        return;
    };
    let captures = state.captures();
    if let Some(session) = captures.sessions.get_mut(id)
        && !session.stopped
        && session.constraints.is_none()
    {
        constraints::send(&session.resource, &initial);
        session.constraints = Some(initial);
    }
}

pub(in crate::capture) fn request<S: CaptureHost>(
    state: &mut S,
    resource: ExtImageCopyCaptureSessionV1,
    request: Request,
) {
    let Request::CreateFrame { frame } = request else {
        return;
    };
    let captures = state.captures();
    let session = resource
        .key::<CaptureSessionId>()
        .filter(|id| captures.sessions.contains_key(*id));
    if let Some(id) = session
        && captures.sessions[id].frame.is_some()
    {
        resource.post_error(Error::DuplicateFrame);
        return;
    }
    let frame_id = captures.frames.insert(Frame {
        resource: frame.clone(),
        session,
        buffer: None,
        damage: Region::new(),
        phase: Phase::Preparing,
    });
    frame.set_key(frame_id);
    if let Some(id) = session {
        captures.sessions[id].frame = Some(frame_id);
    }
}

pub(in crate::capture) fn destroyed<S: CaptureHost>(
    state: &mut S,
    resource: ExtImageCopyCaptureSessionV1,
) {
    let Some(id) = resource.key::<CaptureSessionId>() else {
        return;
    };
    let captures = state.captures();
    let Some(session) = captures.sessions.remove(id) else {
        return;
    };
    if let Some(frame) = session.frame {
        captures.fail(frame, FailureReason::Stopped);
        if let Some(entry) = captures.frames.get_mut(frame) {
            entry.session = None;
        }
    }
}
