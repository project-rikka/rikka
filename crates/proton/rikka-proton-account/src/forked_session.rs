use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::fork_key::ForkKey;
use crate::fork_payload::ForkPayload;
use crate::session_info::SessionInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkedSession {
    #[serde(flatten)]
    session: SessionInfo,
    #[serde(rename = "Payload", default, skip_serializing_if = "Option::is_none")]
    payload: Option<String>,
}

impl ForkedSession {
    #[must_use]
    pub fn session(&self) -> &SessionInfo {
        &self.session
    }

    #[must_use]
    pub fn into_session(self) -> SessionInfo {
        self.session
    }

    #[must_use]
    pub fn sealed_payload(&self) -> Option<&str> {
        self.payload.as_deref()
    }

    pub fn payload(&self, key: &ForkKey) -> Result<ForkPayload> {
        let sealed = self.payload.as_deref().ok_or(Error::NoForkPayload)?;

        ForkPayload::open(key, sealed)
    }
}
