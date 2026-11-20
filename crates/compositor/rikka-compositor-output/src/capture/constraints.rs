use rikka_compositor_protocols::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1;
use rikka_compositor_protocols::wl_shm;
use rikka_compositor_types::Fourcc;

use super::types::Constraints;

pub(super) fn send(session: &ExtImageCopyCaptureSessionV1, constraints: &Constraints) {
    session.buffer_size(
        constraints.size.w.max(0).unsigned_abs(),
        constraints.size.h.max(0).unsigned_abs(),
    );
    for format in constraints
        .formats
        .shm
        .iter()
        .filter_map(|format| shm_format(*format))
    {
        session.shm_format(format);
    }
    if let Some(dmabuf) = &constraints.formats.dmabuf {
        session.dmabuf_device(&dmabuf.device.to_ne_bytes());
        for format in dmabuf.formats.iter() {
            let modifiers: Vec<u8> = format
                .modifiers
                .iter()
                .flat_map(|modifier| modifier.0.to_ne_bytes())
                .collect();
            session.dmabuf_format(format.fourcc.0, &modifiers);
        }
    }
    session.done();
}

fn shm_format(format: Fourcc) -> Option<wl_shm::Format> {
    match format {
        Fourcc::ARGB8888 => Some(wl_shm::Format::Argb8888),
        Fourcc::XRGB8888 => Some(wl_shm::Format::Xrgb8888),
        other => wl_shm::Format::try_from(other.0).ok(),
    }
}
