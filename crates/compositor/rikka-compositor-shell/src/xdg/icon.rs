use rikka_compositor_protocols::wl_buffer::WlBuffer;
use rikka_compositor_protocols::xdg_toplevel_icon_manager_v1::{self, XdgToplevelIconManagerV1};
use rikka_compositor_protocols::xdg_toplevel_icon_v1::{self, Error, XdgToplevelIconV1};
use rikka_compositor_surface::{BufferContent, ShmBuffer};
use rikka_compositor_types::Fourcc;

use super::toplevel_lifecycle::{emit, toplevel_of};
use crate::event::ToplevelEvent;
use crate::host::ShellHost;
use crate::ids::IconDraftId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    pub name: Option<String>,
    pub images: Vec<IconImage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconImage {
    pub size: i32,
    pub scale: i32,
    pub format: Fourcc,
    pub stride: usize,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Default)]
pub(crate) struct Draft {
    name: Option<String>,
    buffers: Vec<(WlBuffer, i32, i32)>,
    assigned: bool,
}

pub(crate) fn announce_sizes(manager: &XdgToplevelIconManagerV1, sizes: &[i32]) {
    for size in sizes {
        manager.icon_size(*size);
    }
    manager.done();
}

pub(crate) fn manager<S: ShellHost>(
    state: &mut S,
    _manager: XdgToplevelIconManagerV1,
    request: xdg_toplevel_icon_manager_v1::Request,
) {
    match request {
        xdg_toplevel_icon_manager_v1::Request::Destroy => {}
        xdg_toplevel_icon_manager_v1::Request::CreateIcon { id } => {
            let draft = state.shell().icons.insert(Draft::default());
            id.set_key(draft);
        }
        xdg_toplevel_icon_manager_v1::Request::SetIcon { toplevel, icon } => {
            let Some(id) = toplevel_of(state, &toplevel) else {
                return;
            };
            let icon = match icon {
                Some(icon) => match assign(state, &icon) {
                    Some(assigned) => assigned,
                    None => return,
                },
                None => None,
            };
            if let Some(record) = state.shell().toplevels.get_mut(id) {
                record.icon = icon;
            }
            emit(state, id, ToplevelEvent::Icon);
        }
    }
}

pub(crate) fn request<S: ShellHost>(
    state: &mut S,
    icon: XdgToplevelIconV1,
    request: xdg_toplevel_icon_v1::Request,
) {
    if matches!(request, xdg_toplevel_icon_v1::Request::Destroy) {
        return;
    }
    let Some(key) = icon.key::<IconDraftId>() else {
        return;
    };
    let size = match &request {
        xdg_toplevel_icon_v1::Request::AddBuffer { buffer, .. } => {
            match state.surfaces().buffer_content(buffer) {
                Some(BufferContent::Shm(shm)) if square(&shm) => Some(shm.size().w),
                _ => None,
            }
        }
        _ => None,
    };
    let Some(draft) = state.shell().icons.get_mut(key) else {
        return;
    };
    if draft.assigned {
        return icon.post_error(Error::Immutable);
    }
    match request {
        xdg_toplevel_icon_v1::Request::Destroy => {}
        xdg_toplevel_icon_v1::Request::SetName { icon_name } => {
            draft.name = Some(icon_name.to_owned());
        }
        xdg_toplevel_icon_v1::Request::AddBuffer { buffer, scale } => {
            let Some(size) = size else {
                return icon.post_error(Error::InvalidBuffer);
            };
            draft.buffers.retain(|(_, known_size, known_scale)| {
                (*known_size, *known_scale) != (size, scale)
            });
            draft.buffers.push((buffer, size, scale));
        }
    }
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, icon: XdgToplevelIconV1) {
    if let Some(key) = icon.key::<IconDraftId>() {
        state.shell().icons.remove(key);
    }
}

fn assign<S: ShellHost>(state: &mut S, icon: &XdgToplevelIconV1) -> Option<Option<Icon>> {
    let draft = state.shell().icons.get_mut(icon.key::<IconDraftId>()?)?;
    draft.assigned = true;
    let (name, buffers) = (draft.name.clone(), draft.buffers.clone());
    let mut images = Vec::with_capacity(buffers.len());
    for (buffer, size, scale) in buffers {
        let Some(image) = read(state, &buffer, size, scale) else {
            icon.post_error(Error::NoBuffer);
            return None;
        };
        images.push(image);
    }
    if name.is_none() && images.is_empty() {
        return Some(None);
    }
    Some(Some(Icon { name, images }))
}

fn read<S: ShellHost>(
    state: &mut S,
    buffer: &WlBuffer,
    size: i32,
    scale: i32,
) -> Option<IconImage> {
    if !buffer.alive() {
        return None;
    }
    let Some(BufferContent::Shm(shm)) = state.surfaces().buffer_content(buffer) else {
        return None;
    };
    let pixels = shm.with_pixels(<[u8]>::to_vec).ok()?;
    Some(IconImage {
        size,
        scale,
        format: shm.format(),
        stride: shm.stride(),
        pixels,
    })
}

fn square(shm: &ShmBuffer<'_>) -> bool {
    let size = shm.size();
    size.w == size.h
}
