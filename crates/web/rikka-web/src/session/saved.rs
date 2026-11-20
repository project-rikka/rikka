use rikka_gecko_protocol::geckoview::session_state::SessionState;

use crate::error::{Error, Result};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavedState(pub(crate) SessionState);

impl SavedState {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.0).expect("session state serializes to JSON")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<SavedState> {
        serde_json::from_slice(bytes)
            .map(SavedState)
            .map_err(Error::State)
    }
}
