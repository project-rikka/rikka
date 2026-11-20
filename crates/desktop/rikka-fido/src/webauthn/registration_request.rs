use std::time::Duration;

use super::attestation::AttestationConveyance;
use super::client_data::{ClientData, ClientDataType};
use super::entities::{Algorithm, RelyingParty, ResidentKey, User, UserVerification};
use super::extensions::{CreateInputs, CredentialProtection, LargeBlobSupport, PrfValues};
use super::origin::Origin;
use super::timeout;
use crate::error::{Error, Result};

const MAX_USER_ID: usize = 64;

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationRequest {
    origin: String,
    rp: RelyingParty,
    user: User,
    challenge: Vec<u8>,
    algorithms: Vec<Algorithm>,
    exclude: Vec<Vec<u8>>,
    resident_key: ResidentKey,
    user_verification: UserVerification,
    timeout: Option<Duration>,
    attestation: AttestationConveyance,
    extensions: CreateInputs,
}

pub(crate) struct PreparedRegistration {
    pub(crate) rp: RelyingParty,
    pub(crate) user: User,
    pub(crate) client_data_json: String,
    pub(crate) client_data_hash: [u8; 32],
    pub(crate) algorithms: Vec<Algorithm>,
    pub(crate) exclude: Vec<Vec<u8>>,
    pub(crate) resident_key: ResidentKey,
    pub(crate) user_verification: UserVerification,
    pub(crate) timeout: Duration,
    pub(crate) attestation: AttestationConveyance,
    pub(crate) extensions: CreateInputs,
}

impl RegistrationRequest {
    pub fn new(
        origin: impl Into<String>,
        rp: RelyingParty,
        user: User,
        challenge: impl Into<Vec<u8>>,
    ) -> RegistrationRequest {
        RegistrationRequest {
            origin: origin.into(),
            rp,
            user,
            challenge: challenge.into(),
            algorithms: vec![Algorithm::ES256, Algorithm::EDDSA, Algorithm::RS256],
            exclude: Vec::new(),
            resident_key: ResidentKey::default(),
            user_verification: UserVerification::default(),
            timeout: None,
            attestation: AttestationConveyance::default(),
            extensions: CreateInputs::default(),
        }
    }

    pub fn algorithms(
        mut self,
        algorithms: impl IntoIterator<Item = Algorithm>,
    ) -> RegistrationRequest {
        self.algorithms = algorithms.into_iter().collect();
        self
    }

    pub fn exclude(mut self, credential_id: impl Into<Vec<u8>>) -> RegistrationRequest {
        self.exclude.push(credential_id.into());
        self
    }

    pub fn resident_key(mut self, resident_key: ResidentKey) -> RegistrationRequest {
        self.resident_key = resident_key;
        self
    }

    pub fn user_verification(mut self, user_verification: UserVerification) -> RegistrationRequest {
        self.user_verification = user_verification;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> RegistrationRequest {
        self.timeout = Some(timeout);
        self
    }

    pub fn attestation(mut self, conveyance: AttestationConveyance) -> RegistrationRequest {
        self.attestation = conveyance;
        self
    }

    pub fn cred_protect(
        mut self,
        policy: CredentialProtection,
        enforce: bool,
    ) -> RegistrationRequest {
        self.extensions.cred_protect = Some((policy, enforce));
        self
    }

    pub fn cred_props(mut self) -> RegistrationRequest {
        self.extensions.cred_props = true;
        self
    }

    pub fn hmac_create_secret(mut self) -> RegistrationRequest {
        self.extensions.hmac_create_secret = true;
        self
    }

    pub fn prf(mut self) -> RegistrationRequest {
        self.extensions.prf.get_or_insert(None);
        self
    }

    pub fn prf_eval(mut self, eval: PrfValues) -> RegistrationRequest {
        self.extensions.prf = Some(Some(eval));
        self
    }

    pub fn min_pin_length(mut self) -> RegistrationRequest {
        self.extensions.min_pin_length = true;
        self
    }

    pub fn large_blob(mut self, support: LargeBlobSupport) -> RegistrationRequest {
        self.extensions.large_blob = Some(support);
        self
    }

    pub(crate) fn prepare(self) -> Result<PreparedRegistration> {
        if self.challenge.is_empty() {
            return Err(Error::InvalidRequest("empty challenge"));
        }
        if self.user.id().is_empty() || self.user.id().len() > MAX_USER_ID {
            return Err(Error::InvalidRequest("user id must be 1 to 64 bytes"));
        }
        if self.algorithms.is_empty() {
            return Err(Error::InvalidRequest("no algorithm is requested"));
        }
        if self.exclude.iter().any(Vec::is_empty) {
            return Err(Error::InvalidRequest("an excluded credential id is empty"));
        }

        let origin = Origin::parse(&self.origin)?;
        let rp_id = origin.validate_rp_id(self.rp.id())?;
        let rp = match self.rp.name() {
            Some(name) => RelyingParty::new(rp_id).with_name(name),
            None => RelyingParty::new(rp_id),
        };

        let client_data =
            ClientData::new(ClientDataType::Create, self.challenge, origin.serialized());

        Ok(PreparedRegistration {
            rp,
            user: self.user,
            client_data_json: client_data.json(),
            client_data_hash: client_data.hash(),
            algorithms: self.algorithms,
            exclude: self.exclude,
            resident_key: self.resident_key,
            user_verification: self.user_verification,
            timeout: timeout::clamp(self.timeout),
            attestation: self.attestation,
            extensions: self.extensions,
        })
    }
}
