use std::rc::Rc;

use rikka_compositor_geometry::{Logical, Point, Region, Transform};
use rikka_compositor_protocols::wl_surface::{Error, Request, WlSurface};
use slotmap::SlotMap;

use super::region::{self, client_rect};
use crate::commit;
use crate::host::SurfaceHost;
use crate::ids::{BufferId, RegionId, SurfaceId};
use crate::state::Pending;

const OFFSET_REQUEST_SINCE: u32 = 5;

pub(crate) fn request<S: SurfaceHost>(state: &mut S, surface: WlSurface, request: Request) {
    let Some(id) = surface.key::<SurfaceId>() else {
        return;
    };
    match request {
        Request::Destroy => {}
        Request::Commit => commit::commit(state, id),
        request => {
            let surfaces = state.surfaces();
            if let Some(target) = surfaces.surfaces.get_mut(id) {
                record(
                    &surface,
                    &mut target.pending,
                    &surfaces.objects.regions,
                    request,
                );
            }
        }
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, surface: WlSurface) {
    if let Some(id) = surface.key::<SurfaceId>() {
        commit::destroy(state, id);
    }
}

fn record(
    surface: &WlSurface,
    pending: &mut Pending,
    regions: &SlotMap<RegionId, Rc<Region<Logical>>>,
    request: Request,
) {
    let update = &mut pending.update;
    match request {
        Request::Destroy | Request::Commit => {}
        Request::Attach { buffer, x, y } => {
            let offset = Point::new(x, y);
            if surface.version() >= OFFSET_REQUEST_SINCE && offset != Point::zero() {
                surface.post_error(Error::InvalidOffset);
                return;
            }
            pending.buffer = Some(buffer.and_then(|buffer| buffer.key::<BufferId>()));
            if surface.version() < OFFSET_REQUEST_SINCE {
                update.offset = offset;
            }
        }
        Request::Offset { x, y } => update.offset = Point::new(x, y),
        Request::Damage {
            x,
            y,
            width,
            height,
        } => update.damage.extend(client_rect(x, y, width, height)),
        Request::DamageBuffer {
            x,
            y,
            width,
            height,
        } => update
            .buffer_damage
            .extend(client_rect(x, y, width, height)),
        Request::Frame { callback } => update.frame_callbacks.push(callback),
        Request::GetRelease { callback } => pending.release_callbacks.push(callback),
        Request::SetOpaqueRegion { region } => {
            update.opaque = Some(
                region
                    .as_ref()
                    .map(|region| region::snapshot(regions, region)),
            );
        }
        Request::SetInputRegion { region } => {
            update.input = Some(
                region
                    .as_ref()
                    .map(|region| region::snapshot(regions, region)),
            );
        }
        Request::SetBufferScale { scale } => {
            if scale < 1 {
                surface.post_error(Error::InvalidScale);
                return;
            }
            update.scale = Some(scale);
        }
        Request::SetBufferTransform { transform } => {
            match transform
                .ok()
                .and_then(|transform| Transform::from_wayland(u32::from(transform)))
            {
                Some(transform) => update.transform = Some(transform),
                None => surface.post_error(Error::InvalidTransform),
            }
        }
    }
}
