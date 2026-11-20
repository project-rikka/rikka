use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::identifier::KeyId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeySalt {
    #[serde(rename = "ID")]
    id: KeyId,
    #[serde(rename = "KeySalt", default, skip_serializing_if = "Option::is_none")]
    salt: Option<String>,
}

impl KeySalt {
    #[must_use]
    pub fn id(&self) -> &KeyId {
        &self.id
    }

    #[must_use]
    pub fn salt(&self) -> Option<&str> {
        self.salt.as_deref().filter(|salt| !salt.is_empty())
    }

    pub fn bytes(&self) -> Result<Vec<u8>> {
        let salt = self
            .salt()
            .ok_or_else(|| Error::NoKeySalt(self.id.clone()))?;

        rikka_proton_api::base64::decode(salt).map_err(|_| Error::MalformedSalt)
    }
}
