use std::rc::Rc;

use rikka_compositor_geometry::{Logical, Region};
use rikka_compositor_protocols::wl_compositor::{Request, WlCompositor};

use crate::host::SurfaceHost;

pub(crate) fn request<S: SurfaceHost>(state: &mut S, _compositor: WlCompositor, request: Request) {
    match request {
        Request::CreateSurface { id } => {
            let surface = state.surfaces().insert(id.clone());
            id.set_key(surface);
        }
        Request::CreateRegion { id } => {
            let region = state
                .surfaces()
                .objects
                .regions
                .insert(Rc::new(Region::<Logical>::new()));
            id.set_key(region);
        }
        Request::Release => {}
    }
}
