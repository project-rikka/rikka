use rikka_wayland_client::wl_shm;
use rikka_wayland_client::{
    ext_image_copy_capture_cursor_session_v1, ext_image_copy_capture_frame_v1,
    ext_image_copy_capture_session_v1,
};

use super::client::Recorded;

#[derive(Debug, Default)]
pub(crate) struct Captured {
    pub(crate) size: Option<(u32, u32)>,
    pub(crate) shm_formats: Vec<wl_shm::Format>,
    pub(crate) described: bool,
    pub(crate) damage: Vec<(i32, i32, i32, i32)>,
    pub(crate) presented: bool,
    pub(crate) ready: Vec<u32>,
    pub(crate) failed: Vec<u32>,
    pub(crate) cursor_entered: bool,
    pub(crate) cursor_at: Option<(i32, i32)>,
    pub(crate) hotspot: Option<(i32, i32)>,
}

fn on_ext_image_copy_capture_session_v1(
    recorded: &mut Recorded,
    _: ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
    event: ext_image_copy_capture_session_v1::Event,
) {
    use ext_image_copy_capture_session_v1::Event;
    match event {
        Event::BufferSize { width, height } => recorded.captured.size = Some((width, height)),
        Event::ShmFormat { format: Ok(format) } => recorded.captured.shm_formats.push(format),
        Event::Done => recorded.captured.described = true,
        _ => {}
    }
}

fn on_ext_image_copy_capture_frame_v1(
    recorded: &mut Recorded,
    proxy: ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1,
    event: ext_image_copy_capture_frame_v1::Event,
) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    use ext_image_copy_capture_frame_v1::Event;
    match event {
        Event::Damage {
            x,
            y,
            width,
            height,
        } => recorded.captured.damage.push((x, y, width, height)),
        Event::PresentationTime { .. } => recorded.captured.presented = true,
        Event::Ready => recorded.captured.ready.push(tag),
        Event::Failed { .. } => recorded.captured.failed.push(tag),
        _ => {}
    }
}

fn on_ext_image_copy_capture_cursor_session_v1(
    recorded: &mut Recorded,
    _: ext_image_copy_capture_cursor_session_v1::ExtImageCopyCaptureCursorSessionV1,
    event: ext_image_copy_capture_cursor_session_v1::Event,
) {
    use ext_image_copy_capture_cursor_session_v1::Event;
    match event {
        Event::Enter => recorded.captured.cursor_entered = true,
        Event::Leave => recorded.captured.cursor_entered = false,
        Event::Position { x, y } => recorded.captured.cursor_at = Some((x, y)),
        Event::Hotspot { x, y } => recorded.captured.hotspot = Some((x, y)),
        _ => {}
    }
}

pub(crate) fn listen(queue: &mut rikka_wayland_client::Queue<Recorded>) {
    queue.on::<ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1>(
        on_ext_image_copy_capture_session_v1,
    );
    queue.on::<ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1>(
        on_ext_image_copy_capture_frame_v1,
    );
    queue.on::<ext_image_copy_capture_cursor_session_v1::ExtImageCopyCaptureCursorSessionV1>(
        on_ext_image_copy_capture_cursor_session_v1,
    );
}
