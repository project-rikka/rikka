use rikka_compositor_geometry::{Buffer, Point, Rect, Size};
use rikka_compositor_protocols::ext_image_copy_capture_frame_v1::{
    Error, ExtImageCopyCaptureFrameV1, FailureReason, Request,
};
use rikka_compositor_protocols::wl_buffer::WlBuffer;
use rikka_compositor_surface::{BufferContent, Surfaces};
use rikka_compositor_types::Fourcc;
use rikka_wayland_wire::Resource;

use crate::capture::host::CaptureHost;
use crate::capture::store::Phase;
use crate::capture::types::{CaptureRequest, Constraints};
use crate::ids::CaptureFrameId;

pub(in crate::capture) fn request<S: CaptureHost>(
    state: &mut S,
    resource: ExtImageCopyCaptureFrameV1,
    request: Request,
) {
    let Some(id) = resource.key::<CaptureFrameId>() else {
        return;
    };
    if matches!(request, Request::Destroy) {
        return;
    }
    let Some(frame) = state.captures().frames.get_mut(id) else {
        return;
    };
    if !matches!(frame.phase, Phase::Preparing) {
        resource.post_error(Error::AlreadyCaptured);
        return;
    }
    match request {
        Request::Destroy => {}
        Request::AttachBuffer { buffer } => frame.buffer = Some(buffer),
        Request::DamageBuffer {
            x,
            y,
            width,
            height,
        } => {
            if x < 0 || y < 0 || width <= 0 || height <= 0 {
                resource.object().post_error(
                    Error::InvalidBufferDamage.code(),
                    &format!("damage {width}x{height}+{x}+{y} is not a region of the buffer"),
                );
                return;
            }
            frame
                .damage
                .add(Rect::new(Point::new(x, y), Size::new(width, height)));
        }
        Request::Capture => capture(state, &resource, id),
    }
}

fn capture<S: CaptureHost>(
    state: &mut S,
    resource: &ExtImageCopyCaptureFrameV1,
    id: CaptureFrameId,
) {
    let Some(buffer) = state.captures().frames[id].buffer.take() else {
        resource.post_error(Error::NoBuffer);
        return;
    };
    let shape = Shape::from_buffer(state.surfaces(), &buffer);
    let captures = state.captures();
    let frame = &mut captures.frames[id];
    let session = frame
        .session
        .and_then(|session| Some((session, captures.sessions.get(session)?)))
        .filter(|(_, session)| !session.stopped);
    let Some((session_id, session)) = session else {
        resource.failed(FailureReason::Stopped);
        frame.phase = Phase::Finished;
        return;
    };
    let Some(constraints) = &session.constraints else {
        resource.failed(FailureReason::BufferConstraints);
        frame.phase = Phase::Finished;
        return;
    };
    if !shape.fits(constraints) {
        resource.failed(FailureReason::BufferConstraints);
        frame.phase = Phase::Finished;
        return;
    }
    let size = constraints.size;
    let info = session.info;
    let damage = std::mem::take(&mut frame.damage);
    frame.phase = Phase::Capturing {
        session: session_id,
        size,
    };
    state.capture_requested(CaptureRequest {
        frame: id,
        session: session_id,
        info,
        size,
        buffer,
        damage,
    });
}

#[derive(Debug, Clone, Copy)]
enum Shape {
    Shm {
        size: Size<i32, Buffer>,
        format: Fourcc,
    },
    Unusable,
    Unchecked,
}

impl Shape {
    fn from_buffer(surfaces: &Surfaces, buffer: &WlBuffer) -> Self {
        match surfaces.buffer_content(buffer) {
            Some(BufferContent::Shm(shm)) => Self::Shm {
                size: shm.size(),
                format: shm.format(),
            },
            Some(BufferContent::SinglePixel(_)) => Self::Unusable,
            _ => Self::Unchecked,
        }
    }

    fn fits(self, constraints: &Constraints) -> bool {
        match self {
            Self::Shm { size, format } => {
                size == constraints.size && constraints.formats.shm.contains(&format)
            }
            Self::Unusable => false,
            Self::Unchecked => true,
        }
    }
}
pub(in crate::capture) fn destroyed<S: CaptureHost>(
    state: &mut S,
    resource: ExtImageCopyCaptureFrameV1,
) {
    let Some(id) = resource.key::<CaptureFrameId>() else {
        return;
    };
    let captures = state.captures();
    let Some(frame) = captures.frames.remove(id) else {
        return;
    };
    if let Some(session) = frame
        .session
        .and_then(|session| captures.sessions.get_mut(session))
        && session.frame == Some(id)
    {
        session.frame = None;
    }
}
