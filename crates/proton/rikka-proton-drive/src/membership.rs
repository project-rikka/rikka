use jiff::Timestamp;
use rikka_proton_account::AddressId;
use serde::Deserialize;

use crate::identifier::MemberId;
use crate::role::Role;

#[derive(Debug, Clone, Deserialize)]
pub struct Membership {
    #[serde(rename = "MemberID")]
    id: MemberId,
    #[serde(rename = "AddressID")]
    address_id: AddressId,
    #[serde(rename = "Inviter", default)]
    inviter: Option<String>,
    #[serde(rename = "Permissions", default)]
    role: Role,
    #[serde(rename = "KeyPacket")]
    key_packet: String,
    #[serde(rename = "KeyPacketSignature", default)]
    key_packet_signature: Option<String>,
    #[serde(rename = "SessionKeySignature", default)]
    session_key_signature: Option<String>,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    created_at: Option<Timestamp>,
}

impl Membership {
    #[must_use]
    pub fn id(&self) -> &MemberId {
        &self.id
    }

    #[must_use]
    pub fn address_id(&self) -> &AddressId {
        &self.address_id
    }

    #[must_use]
    pub fn inviter(&self) -> Option<&str> {
        self.inviter.as_deref()
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    #[must_use]
    pub fn key_packet(&self) -> &str {
        &self.key_packet
    }

    #[must_use]
    pub fn key_packet_signature(&self) -> Option<&str> {
        self.key_packet_signature.as_deref()
    }

    #[must_use]
    pub fn session_key_signature(&self) -> Option<&str> {
        self.session_key_signature.as_deref()
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }
}
