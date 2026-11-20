use super::entities::Algorithm;
use super::extensions::{CreateOutputs, CredentialProtection, PrfOutput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub(crate) credential_id: Vec<u8>,
    pub(crate) client_data_json: String,
    pub(crate) attestation_object: Vec<u8>,
    pub(crate) authenticator_data: Vec<u8>,
    pub(crate) public_key: Vec<u8>,
    pub(crate) algorithm: Algorithm,
    pub(crate) format: String,
    pub(crate) discoverable: bool,
    pub(crate) extensions: CreateOutputs,
}

impl Registration {
    #[must_use]
    pub fn credential_id(&self) -> &[u8] {
        &self.credential_id
    }

    #[must_use]
    pub fn client_data_json(&self) -> &str {
        &self.client_data_json
    }

    #[must_use]
    pub fn attestation_object(&self) -> &[u8] {
        &self.attestation_object
    }

    #[must_use]
    pub fn authenticator_data(&self) -> &[u8] {
        &self.authenticator_data
    }

    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    #[must_use]
    pub fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    #[must_use]
    pub fn format(&self) -> &str {
        &self.format
    }

    #[must_use]
    pub fn is_discoverable(&self) -> bool {
        self.discoverable
    }

    #[must_use]
    pub fn cred_props(&self) -> Option<bool> {
        self.extensions.cred_props
    }

    #[must_use]
    pub fn cred_protect(&self) -> Option<CredentialProtection> {
        self.extensions.cred_protect
    }

    #[must_use]
    pub fn hmac_create_secret(&self) -> Option<bool> {
        self.extensions.hmac_create_secret
    }

    #[must_use]
    pub fn prf(&self) -> Option<&PrfOutput> {
        self.extensions.prf.as_ref()
    }

    #[must_use]
    pub fn min_pin_length(&self) -> Option<u32> {
        self.extensions.min_pin_length
    }

    #[must_use]
    pub fn large_blob_supported(&self) -> Option<bool> {
        self.extensions.large_blob_supported
    }
}
