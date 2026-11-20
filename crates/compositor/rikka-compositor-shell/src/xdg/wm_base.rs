use rikka_compositor_protocols::xdg_wm_base::{Error, Request, XdgWmBase};

use super::{ping, positioner_requests, surface};
use crate::host::ShellHost;
use crate::ids::WmBaseId;

#[derive(Debug)]
pub(crate) struct WmBase {
    pub(crate) resource: XdgWmBase,
    pub(crate) surfaces: usize,
    pub(crate) ping: Option<u32>,
    pub(crate) overdue: bool,
}

pub(crate) fn bound<S: ShellHost>(state: &mut S, wm_base: XdgWmBase) {
    let id = state.shell().wm_bases.insert(WmBase {
        resource: wm_base.clone(),
        surfaces: 0,
        ping: None,
        overdue: false,
    });
    wm_base.set_key(id);
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, wm_base: XdgWmBase) {
    if let Some(id) = wm_base.key::<WmBaseId>() {
        state.shell().wm_bases.remove(id);
    }
}

pub(crate) fn request<S: ShellHost>(state: &mut S, wm_base: XdgWmBase, request: Request) {
    match request {
        Request::Destroy => {
            let defunct = wm_base
                .key::<WmBaseId>()
                .and_then(|id| state.shell().wm_bases.get(id))
                .is_some_and(|record| record.surfaces > 0);
            if defunct {
                wm_base.post_error(Error::DefunctSurfaces);
            }
        }
        Request::CreatePositioner { id } => positioner_requests::create(state, &id),
        Request::GetXdgSurface { id, surface } => surface::create(state, &wm_base, id, &surface),
        Request::Pong { serial } => ping::pong(state, &wm_base, serial),
    }
}
