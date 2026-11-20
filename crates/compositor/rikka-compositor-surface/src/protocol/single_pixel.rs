use rikka_compositor_geometry::Size;
use rikka_compositor_protocols::wp_single_pixel_buffer_manager_v1::{
    Request, WpSinglePixelBufferManagerV1,
};

use crate::buffer::Storage;
use crate::content::Rgba;
use crate::host::SurfaceHost;

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    _manager: WpSinglePixelBufferManagerV1,
    request: Request,
) {
    let Request::CreateU32RgbaBuffer { id, r, g, b, a } = request else {
        return;
    };
    let buffer = state.surfaces().buffers.create(
        id.clone(),
        Storage::SinglePixel(Rgba { r, g, b, a }),
        Size::new(1, 1),
    );
    id.set_key(buffer);
}
