use std::os::fd::AsFd;

use rikka_compositor_protocols::wp_linux_drm_syncobj_manager_v1::{
    self, WpLinuxDrmSyncobjManagerV1,
};
use rikka_compositor_protocols::wp_linux_drm_syncobj_surface_v1::{
    self, WpLinuxDrmSyncobjSurfaceV1,
};
use rikka_compositor_protocols::wp_linux_drm_syncobj_timeline_v1::{
    self, WpLinuxDrmSyncobjTimelineV1,
};

use super::extension::{Claim, claim, target};
use crate::host::SurfaceHost;
use crate::ids::{SurfaceId, TimelineId};
use crate::syncobj::{SyncPoints, Timeline, TimelinePoint};

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    manager: WpLinuxDrmSyncobjManagerV1,
    request: wp_linux_drm_syncobj_manager_v1::Request,
) {
    match request {
        wp_linux_drm_syncobj_manager_v1::Request::Destroy => {}
        wp_linux_drm_syncobj_manager_v1::Request::GetSurface { id, surface } => {
            let claimed = claim(state, &surface, |extensions| {
                extensions.syncobj.is_none() && {
                    extensions.syncobj = Some(id.clone());
                    true
                }
            });
            match claimed {
                Claim::Claimed(surface) => id.set_key(surface),
                Claim::Occupied => {
                    manager.post_error(wp_linux_drm_syncobj_manager_v1::Error::SurfaceExists)
                }
                Claim::Missing => {}
            }
        }
        wp_linux_drm_syncobj_manager_v1::Request::ImportTimeline { id, fd } => {
            let surfaces = state.surfaces();
            let imported = surfaces
                .syncobj
                .clone()
                .and_then(|device| Timeline::import(&device, fd.as_fd()).ok());
            match imported {
                Some(timeline) => id.set_key(surfaces.objects.timelines.insert(timeline)),
                None => manager.post_error(wp_linux_drm_syncobj_manager_v1::Error::InvalidTimeline),
            }
        }
    }
}

pub(crate) fn timeline<S>(
    _state: &mut S,
    _timeline: WpLinuxDrmSyncobjTimelineV1,
    _request: wp_linux_drm_syncobj_timeline_v1::Request,
) {
}

pub(crate) fn timeline_destroyed<S: SurfaceHost>(
    state: &mut S,
    timeline: WpLinuxDrmSyncobjTimelineV1,
) {
    if let Some(id) = timeline.key::<TimelineId>() {
        state.surfaces().objects.timelines.remove(id);
    }
}

pub(crate) fn surface<S: SurfaceHost>(
    state: &mut S,
    object: WpLinuxDrmSyncobjSurfaceV1,
    request: wp_linux_drm_syncobj_surface_v1::Request,
) {
    let (timeline, hi, lo, acquire) = match request {
        wp_linux_drm_syncobj_surface_v1::Request::Destroy => return,
        wp_linux_drm_syncobj_surface_v1::Request::SetAcquirePoint {
            timeline,
            point_hi,
            point_lo,
        } => (timeline, point_hi, point_lo, true),
        wp_linux_drm_syncobj_surface_v1::Request::SetReleasePoint {
            timeline,
            point_hi,
            point_lo,
        } => (timeline, point_hi, point_lo, false),
    };
    let surfaces = state.surfaces();
    let Some(surface) = object
        .key::<SurfaceId>()
        .and_then(|id| surfaces.surfaces.get_mut(id))
    else {
        object.post_error(wp_linux_drm_syncobj_surface_v1::Error::NoSurface);
        return;
    };
    let Some(timeline) = timeline
        .key::<TimelineId>()
        .and_then(|id| surfaces.objects.timelines.get(id))
        .cloned()
    else {
        return;
    };
    let point = TimelinePoint {
        timeline,
        point: (u64::from(hi) << 32) | u64::from(lo),
    };
    let points = &mut surface.pending.points;
    if acquire {
        points.acquire = Some(point);
    } else {
        points.release = Some(point);
    }
}

pub(crate) fn surface_destroyed<S: SurfaceHost>(state: &mut S, object: WpLinuxDrmSyncobjSurfaceV1) {
    if let Some(surface) = target(state, &object)
        && surface.extensions.syncobj.as_ref() == Some(&object)
    {
        surface.extensions.syncobj = None;
        surface.pending.points = SyncPoints::default();
    }
}
