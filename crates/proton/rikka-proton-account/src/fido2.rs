use std::time::Duration;

use rikka_fido::{Assertion, AssertionRequest, UserVerification};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const DEFAULT_RP_ID: &str = "proton.me";
pub(crate) const PUBLIC_KEY_TYPE: &str = "public-key";
const APPID_EXTENSION: &str = "appid";
const FLAG_COMPROMISED: i32 = 0b1;

pub(crate) fn account_origin(rp_id: &str) -> String {
    format!("https://account.{rp_id}")
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fido2Info {
    #[serde(
        rename = "AuthenticationOptions",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    authentication_options: Option<AuthenticationOptions>,
    #[serde(rename = "RegisteredKeys", default)]
    registered_keys: Vec<RegisteredKey>,
}

impl Fido2Info {
    #[must_use]
    pub fn authentication_options(&self) -> Option<&AuthenticationOptions> {
        self.authentication_options.as_ref()
    }

    #[must_use]
    pub fn registered_keys(&self) -> &[RegisteredKey] {
        &self.registered_keys
    }

    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.authentication_options.is_some() && !self.registered_keys.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticationOptions {
    #[serde(rename = "publicKey")]
    public_key: PublicKeyCredentialRequestOptions,
}

impl AuthenticationOptions {
    #[must_use]
    pub fn new(public_key: PublicKeyCredentialRequestOptions) -> AuthenticationOptions {
        AuthenticationOptions { public_key }
    }

    #[must_use]
    pub fn public_key(&self) -> &PublicKeyCredentialRequestOptions {
        &self.public_key
    }

    #[must_use]
    pub fn relying_party(&self) -> &str {
        self.public_key.relying_party().unwrap_or(DEFAULT_RP_ID)
    }

    #[must_use]
    pub fn web_origin(&self) -> String {
        account_origin(self.relying_party())
    }

    pub fn assertion_request(&self, origin: impl Into<String>) -> AssertionRequest {
        let options = &self.public_key;
        let mut request =
            AssertionRequest::new(origin, self.relying_party(), options.challenge.clone())
                .user_verification(
                    options
                        .user_verification()
                        .and_then(UserVerification::from_name)
                        .unwrap_or_default(),
                );

        for credential in options.allow_credentials() {
            if credential.credential_type() == PUBLIC_KEY_TYPE {
                request = request.allow(credential.id());
            }
        }
        if let Some(timeout) = options.timeout() {
            request = request.timeout(Duration::from_millis(timeout));
        }
        if let Some(appid) = options.appid() {
            request = request.appid(appid);
        }
        request
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKeyCredentialRequestOptions {
    #[serde(rename = "challenge")]
    challenge: Vec<u8>,
    #[serde(rename = "timeout", default, skip_serializing_if = "Option::is_none")]
    timeout: Option<u64>,
    #[serde(rename = "rpId", default, skip_serializing_if = "Option::is_none")]
    relying_party: Option<String>,
    #[serde(
        rename = "allowCredentials",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    allow_credentials: Option<Vec<PublicKeyCredentialDescriptor>>,
    #[serde(
        rename = "userVerification",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    user_verification: Option<String>,
    #[serde(
        rename = "extensions",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    extensions: Option<Value>,
}

impl PublicKeyCredentialRequestOptions {
    #[must_use]
    pub fn challenge(&self) -> &[u8] {
        &self.challenge
    }

    #[must_use]
    pub fn timeout(&self) -> Option<u64> {
        self.timeout
    }

    #[must_use]
    pub fn relying_party(&self) -> Option<&str> {
        self.relying_party.as_deref()
    }

    #[must_use]
    pub fn allow_credentials(&self) -> &[PublicKeyCredentialDescriptor] {
        self.allow_credentials.as_deref().unwrap_or_default()
    }

    #[must_use]
    pub fn user_verification(&self) -> Option<&str> {
        self.user_verification.as_deref()
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&Value> {
        self.extensions.as_ref()
    }

    #[must_use]
    pub fn appid(&self) -> Option<&str> {
        self.extensions
            .as_ref()?
            .get(APPID_EXTENSION)?
            .as_str()
            .filter(|appid| !appid.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKeyCredentialDescriptor {
    #[serde(rename = "type")]
    credential_type: String,
    #[serde(rename = "id")]
    id: Vec<u8>,
    #[serde(
        rename = "transports",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    transports: Option<Vec<String>>,
}

impl PublicKeyCredentialDescriptor {
    #[must_use]
    pub fn credential_type(&self) -> &str {
        &self.credential_type
    }

    #[must_use]
    pub fn id(&self) -> &[u8] {
        &self.id
    }

    #[must_use]
    pub fn transports(&self) -> &[String] {
        self.transports.as_deref().unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredKey {
    #[serde(rename = "AttestationFormat")]
    attestation_format: String,
    #[serde(rename = "CredentialID")]
    credential_id: Vec<u8>,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Flags", default)]
    flags: i32,
}

impl RegisteredKey {
    #[must_use]
    pub fn attestation_format(&self) -> &str {
        &self.attestation_format
    }

    #[must_use]
    pub fn credential_id(&self) -> &[u8] {
        &self.credential_id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn is_compromised(&self) -> bool {
        self.flags & FLAG_COMPROMISED != 0
    }
}

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fido2Assertion {
    #[serde(rename = "AuthenticationOptions")]
    authentication_options: AuthenticationOptions,
    #[serde(rename = "ClientData")]
    client_data: String,
    #[serde(rename = "AuthenticatorData")]
    authenticator_data: String,
    #[serde(rename = "Signature")]
    signature: String,
    #[serde(rename = "CredentialID")]
    credential_id: Vec<u8>,
}

impl Fido2Assertion {
    pub fn new(options: AuthenticationOptions) -> Fido2Assertion {
        Fido2Assertion {
            authentication_options: options,
            client_data: String::new(),
            authenticator_data: String::new(),
            signature: String::new(),
            credential_id: Vec::new(),
        }
    }

    pub fn from_assertion(options: AuthenticationOptions, assertion: &Assertion) -> Fido2Assertion {
        Fido2Assertion::new(options)
            .client_data(rikka_proton_api::base64::encode(
                assertion.client_data_json(),
            ))
            .authenticator_data(rikka_proton_api::base64::encode(
                assertion.authenticator_data(),
            ))
            .signature(rikka_proton_api::base64::encode(assertion.signature()))
            .credential_id(assertion.credential_id())
    }

    pub fn client_data(mut self, base64: impl Into<String>) -> Self {
        self.client_data = base64.into();
        self
    }

    pub fn authenticator_data(mut self, base64: impl Into<String>) -> Self {
        self.authenticator_data = base64.into();
        self
    }

    pub fn signature(mut self, base64: impl Into<String>) -> Self {
        self.signature = base64.into();
        self
    }

    pub fn credential_id(mut self, credential_id: impl Into<Vec<u8>>) -> Self {
        self.credential_id = credential_id.into();
        self
    }
}
