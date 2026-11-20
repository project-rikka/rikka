use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::identifier::{AttachmentId, MessageId};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Disposition {
    #[default]
    Attachment,
    Inline,
    Other(String),
}

impl Disposition {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Disposition::Attachment => "attachment",
            Disposition::Inline => "inline",
            Disposition::Other(value) => value,
        }
    }

    #[must_use]
    pub fn is_inline(&self) -> bool {
        matches!(self, Disposition::Inline)
    }
}

impl From<&str> for Disposition {
    fn from(value: &str) -> Disposition {
        match value {
            "attachment" => Disposition::Attachment,
            "inline" => Disposition::Inline,
            other => Disposition::Other(other.to_owned()),
        }
    }
}

impl Serialize for Disposition {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Disposition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Disposition, D::Error> {
        Ok(Disposition::from(
            String::deserialize(deserializer)?.as_str(),
        ))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Attachment {
    #[serde(rename = "ID", default)]
    id: AttachmentId,
    #[serde(rename = "MessageID", default, skip_serializing_if = "Option::is_none")]
    message_id: Option<MessageId>,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Size", default)]
    size: i64,
    #[serde(rename = "MIMEType", default)]
    mime_type: String,
    #[serde(rename = "Disposition", default)]
    disposition: Disposition,
    #[serde(rename = "KeyPackets", default)]
    key_packets: String,
    #[serde(rename = "Signature", default, skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(
        rename = "EncSignature",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    encrypted_signature: Option<String>,
    #[serde(rename = "ContentID", default, skip_serializing_if = "Option::is_none")]
    content_id: Option<String>,
    #[serde(rename = "Headers", default)]
    headers: BTreeMap<String, Value>,
}

impl Attachment {
    #[must_use]
    pub fn id(&self) -> &AttachmentId {
        &self.id
    }

    #[must_use]
    pub fn message_id(&self) -> Option<&MessageId> {
        self.message_id.as_ref()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn size(&self) -> i64 {
        self.size
    }

    #[must_use]
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    #[must_use]
    pub fn disposition(&self) -> &Disposition {
        &self.disposition
    }

    #[must_use]
    pub fn key_packets(&self) -> &str {
        &self.key_packets
    }

    #[must_use]
    pub fn signature(&self) -> Option<&str> {
        self.signature.as_deref()
    }

    #[must_use]
    pub fn encrypted_signature(&self) -> Option<&str> {
        self.encrypted_signature.as_deref()
    }

    #[must_use]
    pub fn headers(&self) -> &BTreeMap<String, Value> {
        &self.headers
    }

    #[must_use]
    pub fn content_id(&self) -> Option<&str> {
        self.content_id.as_deref().or_else(|| {
            self.headers
                .get("content-id")
                .and_then(Value::as_str)
                .map(|value| value.trim_matches(['<', '>']))
        })
    }

    pub fn key_packet_bytes(&self) -> Result<Vec<u8>> {
        if self.key_packets.is_empty() {
            return Err(Error::NoKeyPackets(self.id.clone()));
        }

        rikka_proton_api::base64::decode(&self.key_packets)
            .map_err(|_| Error::NotBase64("attachment key packets"))
    }

    pub fn signature_bytes(&self) -> Result<Option<Vec<u8>>> {
        self.signature
            .as_deref()
            .map(|signature| {
                rikka_proton_api::base64::decode(signature)
                    .map_err(|_| Error::NotBase64("attachment signature"))
            })
            .transpose()
    }
}
