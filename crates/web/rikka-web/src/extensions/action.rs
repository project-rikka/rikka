use rikka_gecko_protocol::geckoview::webextension::action::{Action, ActionPopup, ActionUpdate};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionKind {
    Browser,
    Page,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionChange {
    pub kind: ActionKind,
    pub extension_id: String,
    pub tab: Option<u64>,
    pub action: Action,
}

impl ActionChange {
    pub(crate) fn from_wire(kind: ActionKind, update: ActionUpdate) -> ActionChange {
        ActionChange {
            kind,
            extension_id: update.extension_id,
            tab: update.tab_id,
            action: update.action,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PopupOpening {
    pub kind: ActionKind,
    pub extension_id: String,
    pub popup_uri: Option<String>,
    pub action: Action,
}

impl PopupOpening {
    pub(crate) fn from_wire(kind: ActionKind, popup: ActionPopup) -> PopupOpening {
        PopupOpening {
            kind,
            extension_id: popup.extension_id,
            popup_uri: popup.popup_uri,
            action: popup.action,
        }
    }
}
