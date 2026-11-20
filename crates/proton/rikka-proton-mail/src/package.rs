use std::collections::BTreeMap;
use std::ops::BitOr;

use rikka_proton_pgp::SessionKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::identifier::AttachmentId;
use crate::mime::MimeType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum PackageKind {
    Internal,
    EncryptedOutside,
    Cleartext,
    PgpInline,
    PgpMime,
    ClearMime,
}

impl PackageKind {
    #[must_use]
    pub fn bits(self) -> i32 {
        match self {
            PackageKind::Internal => 1,
            PackageKind::EncryptedOutside => 2,
            PackageKind::Cleartext => 4,
            PackageKind::PgpInline => 8,
            PackageKind::PgpMime => 16,
            PackageKind::ClearMime => 32,
        }
    }

    #[must_use]
    pub fn is_encrypted(self) -> bool {
        matches!(
            self,
            PackageKind::Internal
                | PackageKind::EncryptedOutside
                | PackageKind::PgpInline
                | PackageKind::PgpMime
        )
    }

    #[must_use]
    pub fn is_mime(self) -> bool {
        matches!(self, PackageKind::PgpMime | PackageKind::ClearMime)
    }
}

impl Serialize for PackageKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.bits())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PackageTypes(i32);

impl PackageTypes {
    #[must_use]
    pub fn empty() -> PackageTypes {
        PackageTypes(0)
    }

    #[must_use]
    pub fn bits(self) -> i32 {
        self.0
    }

    #[must_use]
    pub fn contains(self, kind: PackageKind) -> bool {
        self.0 & kind.bits() != 0
    }

    #[must_use]
    pub fn with(self, kind: PackageKind) -> PackageTypes {
        PackageTypes(self.0 | kind.bits())
    }
}

impl BitOr<PackageKind> for PackageTypes {
    type Output = PackageTypes;

    fn bitor(self, kind: PackageKind) -> PackageTypes {
        self.with(kind)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SignatureMode {
    #[default]
    None,
    Detached,
}

impl SignatureMode {
    #[must_use]
    pub fn number(self) -> i32 {
        match self {
            SignatureMode::None => 0,
            SignatureMode::Detached => 1,
        }
    }
}

impl Serialize for SignatureMode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.number())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExposedKey {
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "Algorithm")]
    algorithm: String,
}

impl ExposedKey {
    #[must_use]
    pub fn new(session_key: &SessionKey) -> ExposedKey {
        ExposedKey {
            key: rikka_proton_api::base64::encode(session_key.as_bytes()),
            algorithm: algorithm_name(session_key).to_owned(),
        }
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }
}

#[must_use]
pub(crate) fn algorithm_name(session_key: &SessionKey) -> &'static str {
    match session_key.as_bytes().len() {
        16 => "aes128",
        24 => "aes192",
        _ => "aes256",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutsideAuth {
    #[serde(rename = "Version")]
    version: u8,
    #[serde(rename = "ModulusID")]
    modulus_id: String,
    #[serde(rename = "Salt")]
    salt: String,
    #[serde(rename = "Verifier")]
    verifier: String,
}

impl OutsideAuth {
    #[must_use]
    pub fn new(
        version: u8,
        modulus_id: impl Into<String>,
        salt: impl Into<String>,
        verifier: impl Into<String>,
    ) -> OutsideAuth {
        OutsideAuth {
            version,
            modulus_id: modulus_id.into(),
            salt: salt.into(),
            verifier: verifier.into(),
        }
    }

    #[must_use]
    pub fn version(&self) -> u8 {
        self.version
    }

    #[must_use]
    pub fn modulus_id(&self) -> &str {
        &self.modulus_id
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AddressPackage {
    #[serde(rename = "Type")]
    kind: Option<PackageKind>,
    #[serde(rename = "Signature")]
    signature: SignatureMode,
    #[serde(rename = "BodyKeyPacket", skip_serializing_if = "Option::is_none")]
    body_key_packet: Option<String>,
    #[serde(
        rename = "AttachmentKeyPackets",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    attachment_key_packets: BTreeMap<AttachmentId, String>,
    #[serde(rename = "Token", skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(rename = "EncToken", skip_serializing_if = "Option::is_none")]
    encrypted_token: Option<String>,
    #[serde(rename = "Auth", skip_serializing_if = "Option::is_none")]
    auth: Option<OutsideAuth>,
    #[serde(rename = "PasswordHint", skip_serializing_if = "Option::is_none")]
    password_hint: Option<String>,
}

impl AddressPackage {
    #[must_use]
    pub fn new(kind: PackageKind, signature: SignatureMode) -> AddressPackage {
        AddressPackage {
            kind: Some(kind),
            signature,
            ..AddressPackage::default()
        }
    }

    #[must_use]
    pub fn kind(&self) -> Option<PackageKind> {
        self.kind
    }

    #[must_use]
    pub fn signature(&self) -> SignatureMode {
        self.signature
    }

    #[must_use]
    pub fn body_key_packet(&self) -> Option<&str> {
        self.body_key_packet.as_deref()
    }

    #[must_use]
    pub fn attachment_key_packets(&self) -> &BTreeMap<AttachmentId, String> {
        &self.attachment_key_packets
    }

    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    #[must_use]
    pub fn encrypted_token(&self) -> Option<&str> {
        self.encrypted_token.as_deref()
    }

    #[must_use]
    pub fn auth(&self) -> Option<&OutsideAuth> {
        self.auth.as_ref()
    }

    #[must_use]
    pub fn password_hint(&self) -> Option<&str> {
        self.password_hint.as_deref()
    }

    pub(crate) fn set_body_key_packet(&mut self, packet: Vec<u8>) {
        self.body_key_packet = Some(rikka_proton_api::base64::encode(packet));
    }

    pub(crate) fn add_attachment_key_packet(&mut self, id: AttachmentId, packet: Vec<u8>) {
        self.attachment_key_packets
            .insert(id, rikka_proton_api::base64::encode(packet));
    }

    pub(crate) fn set_outside(
        &mut self,
        token: String,
        encrypted_token: String,
        auth: OutsideAuth,
        hint: Option<String>,
    ) {
        self.token = Some(token);
        self.encrypted_token = Some(encrypted_token);
        self.auth = Some(auth);
        self.password_hint = hint;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Package {
    #[serde(rename = "Addresses")]
    addresses: BTreeMap<String, AddressPackage>,
    #[serde(rename = "MIMEType")]
    mime_type: MimeType,
    #[serde(rename = "Type")]
    kinds: PackageTypes,
    #[serde(rename = "Body")]
    body: String,
    #[serde(rename = "BodyKey", skip_serializing_if = "Option::is_none")]
    body_key: Option<ExposedKey>,
    #[serde(rename = "AttachmentKeys", skip_serializing_if = "BTreeMap::is_empty")]
    attachment_keys: BTreeMap<AttachmentId, ExposedKey>,
}

impl Package {
    #[must_use]
    pub fn new(mime_type: MimeType, body: &[u8]) -> Package {
        Package {
            addresses: BTreeMap::new(),
            mime_type,
            kinds: PackageTypes::empty(),
            body: rikka_proton_api::base64::encode(body),
            body_key: None,
            attachment_keys: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn addresses(&self) -> &BTreeMap<String, AddressPackage> {
        &self.addresses
    }

    #[must_use]
    pub fn address(&self, email: &str) -> Option<&AddressPackage> {
        self.addresses.get(email)
    }

    #[must_use]
    pub fn mime_type(&self) -> &MimeType {
        &self.mime_type
    }

    #[must_use]
    pub fn kinds(&self) -> PackageTypes {
        self.kinds
    }

    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub fn body_key(&self) -> Option<&ExposedKey> {
        self.body_key.as_ref()
    }

    #[must_use]
    pub fn attachment_keys(&self) -> &BTreeMap<AttachmentId, ExposedKey> {
        &self.attachment_keys
    }

    pub(crate) fn add_address(&mut self, email: String, package: AddressPackage) {
        if let Some(kind) = package.kind() {
            self.kinds = self.kinds.with(kind);
        }
        self.addresses.insert(email, package);
    }

    pub(crate) fn expose_body_key(&mut self, session_key: &SessionKey) {
        self.body_key = Some(ExposedKey::new(session_key));
    }

    pub(crate) fn expose_attachment_key(&mut self, id: AttachmentId, session_key: &SessionKey) {
        self.attachment_keys
            .insert(id, ExposedKey::new(session_key));
    }

    pub(crate) fn to_ordered_json(&self, order: &[AttachmentId]) -> Value {
        let mut json = serde_json::to_value(self).expect("package has only string map keys");

        if let Some(keys) = json.get_mut("AttachmentKeys") {
            *keys = ordered_values(keys, order);
        }
        if let Some(Value::Object(addresses)) = json.get_mut("Addresses") {
            for address in addresses.values_mut() {
                if let Some(packets) = address.get_mut("AttachmentKeyPackets") {
                    *packets = ordered_values(packets, order);
                }
            }
        }

        json
    }
}

fn ordered_values(object: &Value, order: &[AttachmentId]) -> Value {
    Value::Array(
        order
            .iter()
            .filter_map(|id| object.get(id.as_str()).cloned())
            .collect(),
    )
}
