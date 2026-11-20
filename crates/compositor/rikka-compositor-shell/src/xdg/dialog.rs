use rikka_compositor_protocols::xdg_dialog_v1::{self, XdgDialogV1};
use rikka_compositor_protocols::xdg_wm_dialog_v1::{self, Error, XdgWmDialogV1};

use super::toplevel_lifecycle::{emit, toplevel_of};
use crate::event::ToplevelEvent;
use crate::host::ShellHost;
use crate::ids::ToplevelId;

pub(crate) fn manager<S: ShellHost>(
    state: &mut S,
    manager: XdgWmDialogV1,
    request: xdg_wm_dialog_v1::Request,
) {
    let xdg_wm_dialog_v1::Request::GetXdgDialog { id, toplevel } = request else {
        return;
    };
    let Some(toplevel) = toplevel_of(state, &toplevel) else {
        return;
    };
    let Some(record) = state.shell().toplevels.get_mut(toplevel) else {
        return;
    };
    if record.dialog.as_ref().is_some_and(XdgDialogV1::alive) {
        return manager.post_error(Error::AlreadyUsed);
    }
    id.set_key(toplevel);
    record.dialog = Some(id);
}

pub(crate) fn request<S: ShellHost>(
    state: &mut S,
    dialog: XdgDialogV1,
    request: xdg_dialog_v1::Request,
) {
    let modal = match request {
        xdg_dialog_v1::Request::Destroy => return,
        xdg_dialog_v1::Request::SetModal => true,
        xdg_dialog_v1::Request::UnsetModal => false,
    };
    set_modal(state, &dialog, modal);
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, dialog: XdgDialogV1) {
    set_modal(state, &dialog, false);
    if let Some(toplevel) = dialog.key::<ToplevelId>()
        && let Some(record) = state.shell().toplevels.get_mut(toplevel)
        && record.dialog.as_ref() == Some(&dialog)
    {
        record.dialog = None;
    }
}

fn set_modal<S: ShellHost>(state: &mut S, dialog: &XdgDialogV1, modal: bool) {
    let Some(toplevel) = dialog.key::<ToplevelId>() else {
        return;
    };
    let Some(record) = state.shell().toplevels.get_mut(toplevel) else {
        return;
    };
    if record.dialog.as_ref() != Some(dialog) || record.modal == modal {
        return;
    }
    record.modal = modal;
    emit(state, toplevel, ToplevelEvent::Modal);
}
