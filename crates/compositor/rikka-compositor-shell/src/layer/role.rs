use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_protocols::zwlr_layer_shell_v1::{self, Layer, ZwlrLayerShellV1};
use rikka_compositor_protocols::zwlr_layer_surface_v1::{Error, ZwlrLayerSurfaceV1};
use rikka_compositor_surface::{
    BufferChange, Committed, PreCommit, Rejected, Role, Surface, SurfaceId, assign_role,
};

use super::surface::{LayerState, LayerSurface};
use crate::event::{LayerEvent, ShellEvent};
use crate::host::ShellHost;
use crate::ids::LayerId;
use crate::lifecycle::{Lifecycle, Transition};
use crate::xdg::PopupParent;

#[derive(Debug)]
pub(crate) struct LayerRole {
    pub(crate) id: LayerId,
}

#[derive(Debug)]
pub(crate) struct CreateParams {
    pub(crate) resource: ZwlrLayerSurfaceV1,
    pub(crate) surface: WlSurface,
    pub(crate) output: Option<WlOutput>,
    pub(crate) layer: Layer,
    pub(crate) namespace: String,
}

pub(crate) fn create<S: ShellHost>(state: &mut S, shell: &ZwlrLayerShellV1, params: CreateParams) {
    let Some(surface) = state.surfaces().lookup(&params.surface) else {
        return;
    };
    if state
        .surfaces()
        .get(surface)
        .is_some_and(Surface::buffer_attached)
    {
        return shell.post_error(zwlr_layer_shell_v1::Error::AlreadyConstructed);
    }
    let initial = LayerState::new(params.layer);
    let id = state.shell().layers.insert(LayerSurface {
        resource: params.resource.clone(),
        surface,
        output: params.output,
        namespace: params.namespace,
        initial,
        lifecycle: Lifecycle::new(initial),
        closed: false,
    });
    if let Err(error) = assign_role(state, surface, LayerRole { id }) {
        state.shell().layers.remove(id);
        return error.post(shell, zwlr_layer_shell_v1::Error::Role.code());
    }
    params.resource.set_key(id);
    emit(state, id, LayerEvent::Created);
}

pub(crate) fn remove<S: ShellHost>(state: &mut S, id: LayerId) {
    if state.shell().layers.remove(id).is_none() {
        return;
    }
    state.shell().dismiss_children(PopupParent::Layer(id));
    emit(state, id, LayerEvent::Destroyed);
}

pub(crate) fn emit<S: ShellHost>(state: &mut S, layer: LayerId, event: LayerEvent) {
    state.shell_event(ShellEvent::Layer { layer, event });
}

impl<S: ShellHost> Role<S> for LayerRole {
    const NAME: &'static str = "zwlr_layer_surface_v1";

    fn pre_commit(state: &mut S, commit: &PreCommit) -> Result<(), Rejected> {
        let Some(id) = state
            .surfaces()
            .role::<LayerRole>(commit.surface)
            .map(|role| role.id)
        else {
            return Ok(());
        };
        let Some(record) = state.shell().layers.get_mut(id) else {
            return Ok(());
        };
        if record.closed {
            return Ok(());
        }
        let error = record.lifecycle.pending.violation().or_else(|| {
            (commit.buffer == BufferChange::Attached && !record.lifecycle.configured())
                .then_some(Error::InvalidSurfaceState)
        });
        if let Some(error) = error {
            record.resource.post_error(error);
            return Err(Rejected);
        }
        record.lifecycle.latch();
        Ok(())
    }

    fn committed(state: &mut S, commit: &Committed) {
        let Some(id) = state
            .surfaces()
            .role::<LayerRole>(commit.surface)
            .map(|role| role.id)
        else {
            return;
        };
        let has_buffer = state
            .surfaces()
            .get(commit.surface)
            .is_some_and(Surface::mapped);
        let Some(record) = state.shell().layers.get_mut(id) else {
            return;
        };
        let before = record.lifecycle.state;
        let initial = record.initial;
        let applied = record.lifecycle.apply(has_buffer, &initial);
        let changed = record.lifecycle.state != before;
        if let Some(serial) = applied.configured {
            emit(state, id, LayerEvent::Configured { serial });
        }
        if changed && applied.transition != Some(Transition::InitialCommit) {
            emit(state, id, LayerEvent::Changed);
        }
        match applied.transition {
            Some(Transition::InitialCommit) => emit(state, id, LayerEvent::InitialCommit),
            Some(Transition::Mapped) => emit(state, id, LayerEvent::Mapped),
            Some(Transition::Unmapped) => {
                state.shell().dismiss_children(PopupParent::Layer(id));
                emit(state, id, LayerEvent::Unmapped);
            }
            None => {}
        }
    }

    fn destroyed(state: &mut S, _surface: SurfaceId, role: Self) {
        remove(state, role.id);
    }
}
