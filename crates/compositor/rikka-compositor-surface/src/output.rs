use rikka_compositor_geometry::Transform;
use rikka_compositor_protocols::wl_output::{self, WlOutput};

use crate::ids::SurfaceId;
use crate::surface::Surface;
use crate::surfaces::Surfaces;

const FRACTIONAL_DENOMINATOR: f64 = 120.0;

impl Surfaces {
    pub fn enter(&mut self, surface: SurfaceId, output: &WlOutput) {
        let Some(surface) = self.surfaces.get_mut(surface) else {
            return;
        };
        surface.outputs.retain(WlOutput::alive);
        if output.client() != surface.client() || surface.outputs.contains(output) {
            return;
        }
        surface.resource.enter(output);
        surface.outputs.push(output.clone());
    }

    pub fn leave(&mut self, surface: SurfaceId, output: &WlOutput) {
        let Some(surface) = self.surfaces.get_mut(surface) else {
            return;
        };
        surface.outputs.retain(WlOutput::alive);
        let Some(index) = surface.outputs.iter().position(|entered| entered == output) else {
            return;
        };
        surface.outputs.swap_remove(index);
        surface.resource.leave(output);
    }

    pub fn set_preferred_scale(&mut self, surface: SurfaceId, scale: f64) {
        let Some(surface) = self.surfaces.get_mut(surface) else {
            return;
        };
        let previous = surface.preferred.scale.replace(scale);
        if previous == Some(scale) {
            return;
        }
        let integer = buffer_scale(scale);
        if previous.map(buffer_scale) != Some(integer) {
            surface.resource.preferred_buffer_scale(integer);
        }
        send_fractional(surface);
    }

    pub fn set_preferred_transform(&mut self, surface: SurfaceId, transform: Transform) {
        let Some(surface) = self.surfaces.get_mut(surface) else {
            return;
        };
        if surface.preferred.transform.replace(transform) == Some(transform) {
            return;
        }
        if let Ok(transform) = wl_output::Transform::try_from(transform.to_wayland()) {
            surface.resource.preferred_buffer_transform(transform);
        }
    }
}

pub(crate) fn send_fractional(surface: &Surface) {
    if let (Some(fractional), Some(scale)) =
        (&surface.extensions.fractional, surface.preferred.scale)
    {
        fractional.preferred_scale((scale * FRACTIONAL_DENOMINATOR).round() as u32);
    }
}

fn buffer_scale(scale: f64) -> i32 {
    scale.ceil().max(1.0) as i32
}
