use zbus::zvariant::{DeserializeDict, OwnedValue, Type};

use crate::backend::cast::CursorMode;
use crate::backend::restore;
use crate::protocol::{CastSource, Persist, PickerRequest, SourceKind};

const MONITOR: u32 = 1;
const WINDOW: u32 = 2;

pub(crate) const SOURCE_TYPES: u32 = MONITOR | WINDOW;

const CURSOR_HIDDEN: u32 = 1;
const CURSOR_EMBEDDED: u32 = 2;
const CURSOR_METADATA: u32 = 4;

pub(crate) const CURSOR_MODES: u32 = CURSOR_HIDDEN | CURSOR_EMBEDDED | CURSOR_METADATA;

const PERSIST_WHILE_RUNNING: u32 = 1;
const PERSIST_UNTIL_REVOKED: u32 = 2;

#[derive(Debug, Default, DeserializeDict, Type)]
#[zvariant(signature = "a{sv}")]
pub(crate) struct SelectOptions {
    types: Option<u32>,
    multiple: Option<bool>,
    cursor_mode: Option<u32>,
    restore_data: Option<(String, u32, OwnedValue)>,
    persist_mode: Option<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceSelection {
    pub(crate) kinds: Vec<SourceKind>,
    pub(crate) multiple: bool,
    pub(crate) cursor: CursorMode,
    pub(crate) persist: Persist,
    pub(crate) restore: Vec<CastSource>,
}

impl SourceSelection {
    pub(crate) fn picker_request(&self, app_id: String) -> PickerRequest {
        PickerRequest {
            app_id,
            kinds: self.kinds.clone(),
            multiple: self.multiple,
            persist: self.persist,
        }
    }
}

impl SelectOptions {
    pub(crate) fn into_selection(self) -> Option<SourceSelection> {
        let cursor = match self.cursor_mode.unwrap_or(CURSOR_HIDDEN) {
            CURSOR_HIDDEN => CursorMode::Hidden,
            CURSOR_EMBEDDED => CursorMode::Embedded,
            CURSOR_METADATA => CursorMode::Metadata,
            _ => return None,
        };
        let types = self.types.unwrap_or(MONITOR) & SOURCE_TYPES;
        let mut kinds = Vec::new();
        if types & MONITOR != 0 {
            kinds.push(SourceKind::Monitor);
        }
        if types & WINDOW != 0 {
            kinds.push(SourceKind::Window);
        }
        if kinds.is_empty() {
            kinds.push(SourceKind::Monitor);
        }

        Some(SourceSelection {
            kinds,
            multiple: self.multiple.unwrap_or(false),
            cursor,
            persist: match self.persist_mode.unwrap_or(0) {
                PERSIST_WHILE_RUNNING => Persist::WhileRunning,
                PERSIST_UNTIL_REVOKED => Persist::UntilRevoked,
                _ => Persist::No,
            },
            restore: self
                .restore_data
                .as_ref()
                .map(restore::read)
                .unwrap_or_default(),
        })
    }
}

pub(crate) fn persist_mode(persist: Persist) -> u32 {
    match persist {
        Persist::No => 0,
        Persist::WhileRunning => PERSIST_WHILE_RUNNING,
        Persist::UntilRevoked => PERSIST_UNTIL_REVOKED,
    }
}

pub(crate) fn source_type(kind: SourceKind) -> u32 {
    match kind {
        SourceKind::Monitor => MONITOR,
        SourceKind::Window => WINDOW,
    }
}
