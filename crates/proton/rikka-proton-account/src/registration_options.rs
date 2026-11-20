use std::time::Duration;

use rikka_fido::{
    Algorithm, AttestationConveyance, CredentialProtection, RegistrationRequest, RelyingParty,
    ResidentKey, User, UserVerification,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::fido2::{DEFAULT_RP_ID, PUBLIC_KEY_TYPE, account_origin};

#[derive(Debug, Clone, PartialEq)]
pub struct RegistrationOptions {
    raw: Value,
    parsed: CreationOptions,
}

impl RegistrationOptions {
    #[must_use]
    pub fn relying_party(&self) -> &str {
        self.parsed.rp.id.as_deref().unwrap_or(DEFAULT_RP_ID)
    }

    #[must_use]
    pub fn relying_party_name(&self) -> &str {
        &self.parsed.rp.name
    }

    #[must_use]
    pub fn challenge(&self) -> &[u8] {
        &self.parsed.challenge
    }

    #[must_use]
    pub fn user_name(&self) -> &str {
        &self.parsed.user.name
    }

    pub fn excluded_credentials(&self) -> impl Iterator<Item = &[u8]> {
        self.parsed
            .exclude_credentials
            .iter()
            .filter(|descriptor| descriptor.credential_type == PUBLIC_KEY_TYPE)
            .map(|descriptor| descriptor.id.as_slice())
    }

    #[must_use]
    pub fn web_origin(&self) -> String {
        account_origin(self.relying_party())
    }

    pub fn registration_request(&self, origin: impl Into<String>) -> RegistrationRequest {
        let options = &self.parsed;

        let rp = RelyingParty::new(self.relying_party()).with_name(options.rp.name.clone());
        let user = User::new(options.user.id.clone())
            .with_name(options.user.name.clone())
            .with_display_name(options.user.display_name.clone());

        let mut request = RegistrationRequest::new(origin, rp, user, options.challenge.clone())
            .algorithms(
                options
                    .parameters
                    .iter()
                    .filter(|parameter| parameter.credential_type == PUBLIC_KEY_TYPE)
                    .map(|parameter| Algorithm::new(parameter.algorithm)),
            );

        for id in self.excluded_credentials() {
            request = request.exclude(id);
        }
        if let Some(timeout) = options.timeout {
            request = request.timeout(Duration::from_millis(timeout));
        }

        let selection = &options.authenticator_selection;
        request = request
            .resident_key(selection.resident_key())
            .user_verification(
                selection
                    .user_verification
                    .as_deref()
                    .and_then(UserVerification::from_name)
                    .unwrap_or_default(),
            );
        request = request.attestation(
            options
                .attestation
                .as_deref()
                .and_then(AttestationConveyance::from_name)
                .unwrap_or_default(),
        );

        let extensions = &options.extensions;
        if extensions.cred_props == Some(true) {
            request = request.cred_props();
        }
        if let Some(policy) = extensions
            .credential_protection_policy
            .as_deref()
            .and_then(CredentialProtection::from_name)
        {
            request = request.cred_protect(
                policy,
                extensions.enforce_credential_protection_policy == Some(true),
            );
        }
        if extensions.hmac_create_secret == Some(true) {
            request = request.hmac_create_secret();
        }
        if extensions.min_pin_length == Some(true) {
            request = request.min_pin_length();
        }
        request
    }
}

impl Serialize for RegistrationOptions {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RegistrationOptions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let parsed = raw
            .get("publicKey")
            .cloned()
            .ok_or_else(|| serde::de::Error::missing_field("publicKey"))
            .and_then(|public_key| {
                CreationOptions::deserialize(public_key).map_err(serde::de::Error::custom)
            })?;
        Ok(RegistrationOptions { raw, parsed })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CreationOptions {
    rp: RelyingPartyEntity,
    user: UserEntity,
    challenge: Vec<u8>,
    #[serde(rename = "pubKeyCredParams")]
    parameters: Vec<CredentialParameter>,
    #[serde(default)]
    timeout: Option<u64>,
    #[serde(rename = "excludeCredentials", default)]
    exclude_credentials: Vec<CredentialDescriptor>,
    #[serde(rename = "authenticatorSelection", default)]
    authenticator_selection: AuthenticatorSelection,
    #[serde(default)]
    attestation: Option<String>,
    #[serde(default)]
    extensions: CreationExtensions,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct RelyingPartyEntity {
    #[serde(default)]
    id: Option<String>,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct UserEntity {
    id: Vec<u8>,
    name: String,
    #[serde(rename = "displayName")]
    display_name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CredentialParameter {
    #[serde(rename = "type")]
    credential_type: String,
    #[serde(rename = "alg")]
    algorithm: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CredentialDescriptor {
    #[serde(rename = "type")]
    credential_type: String,
    id: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
struct AuthenticatorSelection {
    #[serde(rename = "residentKey", default)]
    resident_key: Option<String>,
    #[serde(rename = "requireResidentKey", default)]
    require_resident_key: Option<bool>,
    #[serde(rename = "userVerification", default)]
    user_verification: Option<String>,
}

impl AuthenticatorSelection {
    fn resident_key(&self) -> ResidentKey {
        match self.resident_key.as_deref() {
            Some("required") => ResidentKey::Required,
            Some("preferred") => ResidentKey::Preferred,
            Some("discouraged") => ResidentKey::Discouraged,
            _ if self.require_resident_key == Some(true) => ResidentKey::Required,
            _ => ResidentKey::Discouraged,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
struct CreationExtensions {
    #[serde(rename = "credProps", default)]
    cred_props: Option<bool>,
    #[serde(rename = "credentialProtectionPolicy", default)]
    credential_protection_policy: Option<String>,
    #[serde(rename = "enforceCredentialProtectionPolicy", default)]
    enforce_credential_protection_policy: Option<bool>,
    #[serde(rename = "hmacCreateSecret", default)]
    hmac_create_secret: Option<bool>,
    #[serde(rename = "minPinLength", default)]
    min_pin_length: Option<bool>,
}
