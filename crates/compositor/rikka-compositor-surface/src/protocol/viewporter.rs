use rikka_compositor_geometry::{Point, Rect, Size};
use rikka_compositor_protocols::wp_viewport::{self, WpViewport};
use rikka_compositor_protocols::wp_viewporter::{self, WpViewporter};

use crate::host::SurfaceHost;
use crate::ids::SurfaceId;

const UNSET: f64 = -1.0;

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    viewporter: WpViewporter,
    request: wp_viewporter::Request,
) {
    let wp_viewporter::Request::GetViewport { id, surface } = request else {
        return;
    };
    let surfaces = state.surfaces();
    let Some(target_id) = surfaces.lookup(&surface) else {
        return;
    };
    let Some(target) = surfaces.surfaces.get_mut(target_id) else {
        return;
    };
    if target.extensions.viewport.is_some() {
        viewporter.post_error(wp_viewporter::Error::ViewportExists);
        return;
    }
    target.extensions.viewport = Some(id.clone());
    id.set_key(target_id);
}

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    viewport: WpViewport,
    request: wp_viewport::Request,
) {
    if let wp_viewport::Request::Destroy = request {
        return;
    }
    let Some(surface) = viewport
        .key::<SurfaceId>()
        .and_then(|surface| state.surfaces().surfaces.get_mut(surface))
    else {
        viewport.post_error(wp_viewport::Error::NoSurface);
        return;
    };
    let update = &mut surface.pending.update;
    match request {
        wp_viewport::Request::Destroy => {}
        wp_viewport::Request::SetSource {
            x,
            y,
            width,
            height,
        } => {
            if [x, y, width, height] == [UNSET; 4] {
                update.source = Some(None);
            } else if x < 0.0 || y < 0.0 || width <= 0.0 || height <= 0.0 {
                viewport.post_error(wp_viewport::Error::BadValue);
            } else {
                update.source = Some(Some(Rect::new(Point::new(x, y), Size::new(width, height))));
            }
        }
        wp_viewport::Request::SetDestination { width, height } => {
            if (width, height) == (-1, -1) {
                update.destination = Some(None);
            } else if width <= 0 || height <= 0 {
                viewport.post_error(wp_viewport::Error::BadValue);
            } else {
                update.destination = Some(Some(Size::new(width, height)));
            }
        }
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, viewport: WpViewport) {
    let Some(surface) = viewport
        .key::<SurfaceId>()
        .and_then(|surface| state.surfaces().surfaces.get_mut(surface))
    else {
        return;
    };
    if surface.extensions.viewport.as_ref() != Some(&viewport) {
        return;
    }
    surface.extensions.viewport = None;
    surface.pending.update.source = Some(None);
    surface.pending.update.destination = Some(None);
}
