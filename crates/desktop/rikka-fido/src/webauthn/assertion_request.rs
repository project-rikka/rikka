use std::time::Duration;

use super::client_data::{ClientData, ClientDataType};
use super::entities::UserVerification;
use super::extensions::{GetInputs, PrfValues, Salts};
use super::origin::Origin;
use super::timeout;
use crate::error::{Error, Result};

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssertionRequest {
    origin: String,
    rp_id: String,
    challenge: Vec<u8>,
    allow: Vec<Vec<u8>>,
    user_verification: UserVerification,
    timeout: Option<Duration>,
    appid: Option<String>,
    extensions: GetInputs,
}

pub(crate) struct PreparedAssertion {
    pub(crate) rp_id: String,
    pub(crate) appid: Option<String>,
    pub(crate) client_data_json: String,
    pub(crate) client_data_hash: [u8; 32],
    pub(crate) allow: Vec<Vec<u8>>,
    pub(crate) user_verification: UserVerification,
    pub(crate) timeout: Duration,
    pub(crate) extensions: GetInputs,
}

impl AssertionRequest {
    pub fn new(
        origin: impl Into<String>,
        rp_id: impl Into<String>,
        challenge: impl Into<Vec<u8>>,
    ) -> AssertionRequest {
        AssertionRequest {
            origin: origin.into(),
            rp_id: rp_id.into(),
            challenge: challenge.into(),
            allow: Vec::new(),
            user_verification: UserVerification::default(),
            timeout: None,
            appid: None,
            extensions: GetInputs::default(),
        }
    }

    pub fn allow(mut self, credential_id: impl Into<Vec<u8>>) -> AssertionRequest {
        self.allow.push(credential_id.into());
        self
    }

    pub fn user_verification(mut self, user_verification: UserVerification) -> AssertionRequest {
        self.user_verification = user_verification;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> AssertionRequest {
        self.timeout = Some(timeout);
        self
    }

    pub fn appid(mut self, appid: impl Into<String>) -> AssertionRequest {
        self.appid = Some(appid.into());
        self
    }

    pub fn prf(mut self, eval: PrfValues) -> AssertionRequest {
        self.extensions.prf = Some(eval);
        self
    }

    pub fn prf_for(
        mut self,
        credential_id: impl Into<Vec<u8>>,
        eval: PrfValues,
    ) -> AssertionRequest {
        self.extensions
            .prf_by_credential
            .push((credential_id.into(), eval));
        self
    }

    pub fn hmac_secret(mut self, first: [u8; 32], second: Option<[u8; 32]>) -> AssertionRequest {
        self.extensions.hmac_secret = Some(Salts { first, second });
        self
    }

    pub fn large_blob_read(mut self) -> AssertionRequest {
        self.extensions.large_blob_read = true;
        self
    }

    #[must_use]
    pub fn rp_id(&self) -> &str {
        &self.rp_id
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub(crate) fn prepare(self) -> Result<PreparedAssertion> {
        if self.challenge.is_empty() {
            return Err(Error::InvalidRequest("empty challenge"));
        }
        if self.allow.iter().any(Vec::is_empty) {
            return Err(Error::InvalidRequest("an allowed credential id is empty"));
        }
        self.check_extensions()?;

        let origin = Origin::parse(&self.origin)?;
        let rp_id = origin.validate_rp_id(&self.rp_id)?;
        let appid = self
            .appid
            .map(|appid| origin.validate_appid(&appid))
            .transpose()?;

        let client_data = ClientData::new(ClientDataType::Get, self.challenge, origin.serialized());

        Ok(PreparedAssertion {
            rp_id,
            appid,
            client_data_json: client_data.json(),
            client_data_hash: client_data.hash(),
            allow: self.allow,
            user_verification: self.user_verification,
            timeout: timeout::clamp(self.timeout),
            extensions: self.extensions,
        })
    }

    fn check_extensions(&self) -> Result<()> {
        let extensions = &self.extensions;
        let uses_prf = extensions.prf.is_some() || !extensions.prf_by_credential.is_empty();
        if uses_prf && extensions.hmac_secret.is_some() {
            return Err(Error::InvalidRequest(
                "prf and hmac-secret are mutually exclusive",
            ));
        }
        if !extensions.prf_by_credential.is_empty() && self.allow.is_empty() {
            return Err(Error::InvalidRequest(
                "prf evaluations by credential need an allow list",
            ));
        }
        let unlisted = extensions
            .prf_by_credential
            .iter()
            .any(|(id, _)| !self.allow.contains(id));
        if unlisted {
            return Err(Error::InvalidRequest(
                "prf evaluation references a credential outside allowCredentials",
            ));
        }
        Ok(())
    }
}
