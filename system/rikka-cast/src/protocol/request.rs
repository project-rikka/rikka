use std::io::Read;

use rikka_portal::Result;
use serde::{Deserialize, Serialize};

use crate::protocol::source::SourceKind;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Persist {
    #[default]
    No,
    WhileRunning,
    UntilRevoked,
}

impl Persist {
    pub(crate) fn is_offered(self) -> bool {
        self != Persist::No
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct PickerRequest {
    #[serde(default)]
    pub(crate) app_id: String,
    pub(crate) kinds: Vec<SourceKind>,
    #[serde(default)]
    pub(crate) multiple: bool,
    #[serde(default)]
    pub(crate) persist: Persist,
}

impl PickerRequest {
    pub(crate) fn offers(&self, kind: SourceKind) -> bool {
        self.kinds.contains(&kind)
    }

    pub(crate) fn read(reader: impl Read) -> Result<PickerRequest> {
        rikka_portal::read(reader)
    }
}
