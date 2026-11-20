use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_protocols::wl_data_source::{Error, Request, WlDataSource};

use crate::access::Access;
use crate::drag;
use crate::ids::SourceId;
use crate::publish;

pub(crate) fn source_request<A: Access>(state: &mut A, source: WlDataSource, request: Request) {
    let Some(id) = source.key::<SourceId>() else {
        return;
    };
    let Some(record) = state.selections().sources.get_mut(id) else {
        return;
    };
    match request {
        Request::Offer { mime_type } => record.offer(mime_type),
        Request::SetActions { dnd_actions } => {
            if !DndAction::all().contains(dnd_actions) {
                return source.post_error(Error::InvalidActionMask);
            }
            if record.actions.is_some() || !record.is_fresh() {
                return source.post_error(Error::InvalidSource);
            }
            record.actions = Some(dnd_actions);
        }
        Request::Destroy => {}
    }
}

pub(crate) fn source_destroyed<A: Access>(state: &mut A, source: WlDataSource) {
    let Some(id) = source.key::<SourceId>() else {
        return;
    };
    drag::source_gone(state, id);
    publish::source_gone(state, id);
}
