use rikka_seam::net::SocketClient;
use rikka_seam::{Request, Version};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const ASSISTANT: &str = "rikka.assistant";

pub const ASSISTANT_VERSION: Version = Version::new(1, 1);

pub const OPEN_KIT: &str = "summon";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRequest {
    kit: String,
}

impl OpenRequest {
    #[must_use]
    pub fn new(kit: impl Into<String>) -> OpenRequest {
        OpenRequest { kit: kit.into() }
    }

    #[must_use]
    pub fn kit(&self) -> &str {
        &self.kit
    }
}

pub fn open_assistant(kit: &str) -> Result<()> {
    let socket = SocketClient::for_capability(ASSISTANT)
        .map_err(|error| Error::Unreachable(error.to_string()))?;
    socket
        .client()
        .call(
            Request::new(ASSISTANT, OPEN_KIT)
                .require(ASSISTANT_VERSION)
                .encode(&OpenRequest::new(kit)),
        )
        .blocking_wait()
        .map_err(|error| Error::Unreachable(error.to_string()))?;
    Ok(())
}
