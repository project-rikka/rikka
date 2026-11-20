use rikka_seam::{Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.activation", Version::new(1, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const MINT: &str = "mint";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
}

impl ActivationRequest {
    #[must_use]
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            app_id: Some(app_id.into()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationToken {
    pub token: String,
}

#[derive(Clone, Debug)]
pub struct ActivationClient {
    seam: SeamClient,
}

impl ActivationClient {
    #[must_use]
    pub fn new(seam: SeamClient) -> Self {
        Self { seam }
    }

    pub fn mint(&self, app_id: impl Into<String>) -> Reply {
        PROTOCOL.call_with(&self.seam, MINT, &ActivationRequest::new(app_id))
    }

    pub fn mint_unattributed(&self) -> Reply {
        PROTOCOL.call_with(&self.seam, MINT, &ActivationRequest::default())
    }
}
