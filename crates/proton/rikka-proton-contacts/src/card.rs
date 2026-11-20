use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

wire_enum! {
    #[non_exhaustive]
    pub enum CardType {
        Clear = 0,
        Encrypted = 1,
        Signed = 2,
        EncryptedAndSigned = 3,
    }
}

impl CardType {
    #[must_use]
    pub fn is_encrypted(self) -> bool {
        matches!(self, CardType::Encrypted | CardType::EncryptedAndSigned)
    }

    #[must_use]
    pub fn is_signed(self) -> bool {
        matches!(self, CardType::Signed | CardType::EncryptedAndSigned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactCard {
    #[serde(rename = "Type")]
    kind: CardType,
    #[serde(rename = "Data")]
    data: String,
    #[serde(rename = "Signature", default)]
    signature: Option<String>,
}

impl ContactCard {
    #[must_use]
    pub fn new(kind: CardType, data: impl Into<String>, signature: Option<String>) -> ContactCard {
        ContactCard {
            kind,
            data: data.into(),
            signature: signature.filter(|signature| !signature.is_empty()),
        }
    }

    #[must_use]
    pub fn kind(&self) -> CardType {
        self.kind
    }

    #[must_use]
    pub fn data(&self) -> &str {
        &self.data
    }

    #[must_use]
    pub fn signature(&self) -> Option<&str> {
        self.signature.as_deref()
    }
}
