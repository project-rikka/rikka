use std::rc::Rc;

use rikka_compositor_geometry::{Logical, Point, Rect, Region, Size};
use rikka_compositor_protocols::wl_region::{Request, WlRegion};
use slotmap::SlotMap;

use crate::host::SurfaceHost;
use crate::ids::RegionId;

pub(crate) fn request<S: SurfaceHost>(state: &mut S, region: WlRegion, request: Request) {
    let Some(id) = region.key::<RegionId>() else {
        return;
    };
    let Some(target) = state.surfaces().objects.regions.get_mut(id) else {
        return;
    };
    match request {
        Request::Destroy => {}
        Request::Add {
            x,
            y,
            width,
            height,
        } => {
            if let Some(rect) = client_rect(x, y, width, height) {
                Rc::make_mut(target).add(rect);
            }
        }
        Request::Subtract {
            x,
            y,
            width,
            height,
        } => {
            if let Some(rect) = client_rect(x, y, width, height) {
                Rc::make_mut(target).subtract(rect);
            }
        }
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, region: WlRegion) {
    if let Some(id) = region.key::<RegionId>() {
        state.surfaces().objects.regions.remove(id);
    }
}

pub(super) fn client_rect<S>(x: i32, y: i32, width: i32, height: i32) -> Option<Rect<i32, S>> {
    (width > 0 && height > 0).then(|| Rect::new(Point::new(x, y), Size::new(width, height)))
}

pub(crate) fn snapshot(
    regions: &SlotMap<RegionId, Rc<Region<Logical>>>,
    region: &WlRegion,
) -> Rc<Region<Logical>> {
    region
        .key::<RegionId>()
        .and_then(|id| regions.get(id))
        .cloned()
        .unwrap_or_default()
}
