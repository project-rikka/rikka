use rikka_types::{DragAction, DragActions};
use rikka_wayland_client::wl_data_device_manager::DndAction;

pub(crate) fn wire(action: DragAction) -> DndAction {
    match action {
        DragAction::Copy => DndAction::COPY,
        DragAction::Move => DndAction::MOVE,
        DragAction::Ask => DndAction::ASK,
    }
}

pub(crate) fn action_of(action: DndAction) -> Option<DragAction> {
    DragAction::ALL
        .into_iter()
        .find(|candidate| action == wire(*candidate))
}

pub(crate) fn wire_all(actions: DragActions) -> DndAction {
    DragAction::ALL
        .into_iter()
        .filter(|action| actions.contains(*action))
        .fold(DndAction::empty(), |all, action| all | wire(action))
}

pub(crate) fn actions_of(all: DndAction) -> DragActions {
    DragAction::ALL
        .into_iter()
        .filter(|action| all.contains(wire(*action)))
        .fold(DragActions::NONE, |actions, action| actions | action.into())
}
