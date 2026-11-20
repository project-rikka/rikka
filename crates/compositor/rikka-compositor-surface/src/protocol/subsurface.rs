use rikka_compositor_geometry::Point;
use rikka_compositor_protocols::wl_subcompositor::{self, WlSubcompositor};
use rikka_compositor_protocols::wl_subsurface::{self, WlSubsurface};
use rikka_compositor_protocols::wl_surface::WlSurface;

use crate::commit;
use crate::host::SurfaceHost;
use crate::ids::SurfaceId;
use crate::role::assign_role;
use crate::state::Placement;
use crate::subsurface::Subsurface;
use crate::surfaces::Surfaces;

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    subcompositor: WlSubcompositor,
    request: wl_subcompositor::Request,
) {
    let wl_subcompositor::Request::GetSubsurface {
        id,
        surface,
        parent,
    } = request
    else {
        return;
    };
    let surfaces = state.surfaces();
    let (Some(child), Some(parent)) = (surfaces.lookup(&surface), surfaces.lookup(&parent)) else {
        return;
    };
    if child == parent {
        subcompositor.post_error(wl_subcompositor::Error::BadSurface);
        return;
    }
    if surfaces.is_ancestor(child, parent) {
        subcompositor.post_error(wl_subcompositor::Error::BadParent);
        return;
    }
    if let Err(error) = assign_role(state, child, Subsurface) {
        error.post(&subcompositor, wl_subcompositor::Error::BadSurface.code());
        return;
    }
    state.surfaces().link(child, parent);
    id.set_key(child);
}

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    subsurface: WlSubsurface,
    request: wl_subsurface::Request,
) {
    let Some(child) = subsurface.key::<SurfaceId>() else {
        return;
    };
    let surfaces = state.surfaces();
    if surfaces.role::<Subsurface>(child).is_none() {
        return;
    }
    let Some(parent) = surfaces.get(child).and_then(|surface| surface.parent) else {
        return;
    };
    match request {
        wl_subsurface::Request::Destroy => {}
        wl_subsurface::Request::SetPosition { x, y } => {
            if let Some(parent) = surfaces.surfaces.get_mut(parent) {
                parent.pending.update.position(child, Point::new(x, y));
            }
        }
        wl_subsurface::Request::PlaceAbove { sibling } => {
            let Some(sibling) = sibling_of(surfaces, &subsurface, child, parent, &sibling) else {
                return;
            };
            place(surfaces, parent, Placement::Above { child, sibling });
        }
        wl_subsurface::Request::PlaceBelow { sibling } => {
            let Some(sibling) = sibling_of(surfaces, &subsurface, child, parent, &sibling) else {
                return;
            };
            place(surfaces, parent, Placement::Below { child, sibling });
        }
        wl_subsurface::Request::SetSync => {
            if let Some(surface) = surfaces.surfaces.get_mut(child) {
                surface.sync = true;
            }
        }
        wl_subsurface::Request::SetDesync => {
            let synced = surfaces.synced(child);
            if let Some(surface) = surfaces.surfaces.get_mut(child) {
                surface.sync = false;
            }
            if synced && !state.surfaces().synced(child) {
                commit::flush_cached(state, child);
            }
        }
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, subsurface: WlSubsurface) {
    if let Some(child) = subsurface.key::<SurfaceId>() {
        state.surfaces().detach(child);
    }
}

fn sibling_of(
    surfaces: &Surfaces,
    subsurface: &WlSubsurface,
    child: SurfaceId,
    parent: SurfaceId,
    sibling: &WlSurface,
) -> Option<SurfaceId> {
    let sibling = surfaces.lookup(sibling).filter(|sibling| {
        *sibling == parent
            || (*sibling != child
                && surfaces.get(*sibling).and_then(|surface| surface.parent) == Some(parent))
    });
    if sibling.is_none() {
        subsurface.post_error(wl_subsurface::Error::BadSurface);
    }
    sibling
}

fn place(surfaces: &mut Surfaces, parent: SurfaceId, placement: Placement) {
    if let Some(parent) = surfaces.surfaces.get_mut(parent) {
        parent.pending.update.placements.push(placement);
    }
}
