use rikka_compositor_protocols::wp_color_representation_surface_v1::Error;

use super::representation::compatible;
use crate::latest::Latest;
use crate::surface::Surface;
use crate::surfaces::Surfaces;

impl Surfaces {
    pub(crate) fn representation_valid(&self, surface: &Surface, latest: &Latest) -> bool {
        if surface.extensions.representation.is_none() {
            return true;
        }
        let Some(format) = latest.buffer.and_then(|buffer| self.buffers.format(buffer)) else {
            return true;
        };
        if compatible(&latest.representation, format) {
            return true;
        }
        if let Some(object) = &surface.extensions.representation {
            object.post_error(Error::PixelFormat);
        }
        false
    }
}
