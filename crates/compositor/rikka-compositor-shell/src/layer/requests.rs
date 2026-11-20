use rikka_compositor_geometry::Size;
use rikka_compositor_protocols::zwlr_layer_shell_v1::{self, ZwlrLayerShellV1};
use rikka_compositor_protocols::zwlr_layer_surface_v1::{
    Anchor, Error, KeyboardInteractivity, Request, ZwlrLayerSurfaceV1,
};
use rikka_wayland_wire::Resource;

use super::role::{self, CreateParams};
use super::surface::LayerState;
use crate::host::ShellHost;
use crate::ids::LayerId;
use crate::xdg::{PopupParent, popup_of};

const ON_DEMAND_SINCE: u32 = 4;

pub(crate) fn shell<S: ShellHost>(
    state: &mut S,
    shell: ZwlrLayerShellV1,
    request: zwlr_layer_shell_v1::Request,
) {
    let zwlr_layer_shell_v1::Request::GetLayerSurface {
        id,
        surface,
        output,
        layer,
        namespace,
    } = request
    else {
        return;
    };
    let Ok(layer) = layer else {
        return shell.post_error(zwlr_layer_shell_v1::Error::InvalidLayer);
    };
    role::create(
        state,
        &shell,
        CreateParams {
            resource: id,
            surface,
            output,
            layer,
            namespace: namespace.to_owned(),
        },
    );
}

pub(crate) fn request<S: ShellHost>(state: &mut S, layer: ZwlrLayerSurfaceV1, request: Request) {
    let Some(id) = layer
        .key::<LayerId>()
        .filter(|id| state.shell().layers.contains_key(*id))
    else {
        return;
    };
    match request {
        Request::Destroy => {}
        Request::SetSize { width, height } => edit(state, id, |pending| {
            pending.size = Size::new(saturate(width), saturate(height));
        }),
        Request::SetAnchor { anchor } => {
            if !Anchor::all().contains(anchor) {
                return layer.post_error(Error::InvalidAnchor);
            }
            edit(state, id, |pending| pending.anchor = anchor);
        }
        Request::SetExclusiveZone { zone } => {
            edit(state, id, |pending| pending.exclusive_zone = zone);
        }
        Request::SetMargin {
            top,
            right,
            bottom,
            left,
        } => edit(state, id, |pending| {
            pending.margins = super::surface::Margins {
                top,
                right,
                bottom,
                left,
            };
        }),
        Request::SetKeyboardInteractivity {
            keyboard_interactivity,
        } => match keyboard_interactivity {
            Ok(KeyboardInteractivity::OnDemand) if layer.version() < ON_DEMAND_SINCE => {
                layer.post_error(Error::InvalidKeyboardInteractivity);
            }
            Ok(interactivity) => {
                edit(state, id, |pending| {
                    pending.keyboard_interactivity = interactivity;
                });
            }
            Err(_) => layer.post_error(Error::InvalidKeyboardInteractivity),
        },
        Request::GetPopup { popup } => {
            let Some(popup) = popup_of(state, &popup) else {
                return;
            };
            if let Some(record) = state.shell().popups.get_mut(popup)
                && record.parent.is_none()
                && !record.lifecycle.initialized()
            {
                record.parent = Some(PopupParent::Layer(id));
            }
        }
        Request::AckConfigure { serial } => {
            let acked = state
                .shell()
                .layers
                .get_mut(id)
                .is_none_or(|record| record.lifecycle.ack(serial));
            if !acked {
                layer.post_error(Error::InvalidSurfaceState);
            }
        }
        Request::SetLayer { layer: requested } => match requested {
            Ok(requested) => edit(state, id, |pending| pending.layer = requested),
            Err(_) => layer.object().post_error(
                zwlr_layer_shell_v1::Error::InvalidLayer.code(),
                "invalid layer",
            ),
        },
        Request::SetExclusiveEdge { edge } => {
            if edge.bits().count_ones() > 1 || !Anchor::all().contains(edge) {
                return layer.post_error(Error::InvalidExclusiveEdge);
            }
            edit(state, id, |pending| pending.exclusive_edge = edge);
        }
    }
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, layer: ZwlrLayerSurfaceV1) {
    let Some(id) = layer.key::<LayerId>() else {
        return;
    };
    let Some(surface) = state.shell().layers.get(id).map(|record| record.surface) else {
        return;
    };
    state.surfaces().end_role::<super::role::LayerRole>(surface);
    role::remove(state, id);
}

fn edit<S: ShellHost>(state: &mut S, id: LayerId, change: impl FnOnce(&mut LayerState)) {
    if let Some(record) = state.shell().layers.get_mut(id) {
        change(&mut record.lifecycle.pending);
    }
}

fn saturate(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
