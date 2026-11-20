use serde::{Deserialize, Serialize};

use crate::fork_key::ForkKey;

const SIGN_IN_CODE_VERSION: u8 = 0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkInvitation {
    #[serde(rename = "Selector")]
    selector: String,
    #[serde(rename = "UserCode")]
    user_code: String,
}

impl ForkInvitation {
    #[must_use]
    pub fn selector(&self) -> &str {
        &self.selector
    }

    #[must_use]
    pub fn user_code(&self) -> &str {
        &self.user_code
    }

    #[must_use]
    pub fn sign_in_code(&self, key: &ForkKey, child_client_id: &str) -> String {
        format!(
            "{SIGN_IN_CODE_VERSION}:{}:{}:{child_client_id}",
            self.user_code,
            key.to_base64()
        )
    }
}
