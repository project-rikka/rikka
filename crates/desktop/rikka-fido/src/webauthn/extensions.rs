use crate::crypto;

const PRF_CONTEXT: &[u8] = b"WebAuthn PRF\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrfValues {
    first: Vec<u8>,
    second: Option<Vec<u8>>,
}

impl PrfValues {
    #[must_use]
    pub fn new(first: impl Into<Vec<u8>>) -> PrfValues {
        PrfValues {
            first: first.into(),
            second: None,
        }
    }

    #[must_use]
    pub fn with_second(mut self, second: impl Into<Vec<u8>>) -> PrfValues {
        self.second = Some(second.into());
        self
    }

    #[must_use]
    pub fn first(&self) -> &[u8] {
        &self.first
    }

    #[must_use]
    pub fn second(&self) -> Option<&[u8]> {
        self.second.as_deref()
    }

    pub(crate) fn salts(&self) -> Salts {
        let salt = |input: &[u8]| crypto::sha256([PRF_CONTEXT, input].concat());
        Salts {
            first: salt(&self.first),
            second: self.second.as_deref().map(salt),
        }
    }

    pub(crate) fn from_outputs(outputs: &[u8]) -> PrfValues {
        let (first, second) = outputs.split_at(outputs.len().min(32));
        PrfValues {
            first: first.to_vec(),
            second: (!second.is_empty()).then(|| second.to_vec()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Salts {
    pub(crate) first: [u8; 32],
    pub(crate) second: Option<[u8; 32]>,
}

impl Salts {
    pub(crate) fn joined(&self) -> Vec<u8> {
        let mut joined = self.first.to_vec();
        if let Some(second) = &self.second {
            joined.extend_from_slice(second);
        }
        joined
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrfOutput {
    pub(crate) enabled: bool,
    pub(crate) results: Option<PrfValues>,
}

impl PrfOutput {
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn results(&self) -> Option<&PrfValues> {
        self.results.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialProtection {
    UserVerificationOptional,
    UserVerificationOptionalWithCredentialIdList,
    UserVerificationRequired,
}

impl CredentialProtection {
    #[must_use]
    pub fn from_name(name: &str) -> Option<CredentialProtection> {
        match name {
            "userVerificationOptional" => Some(CredentialProtection::UserVerificationOptional),
            "userVerificationOptionalWithCredentialIDList" => {
                Some(CredentialProtection::UserVerificationOptionalWithCredentialIdList)
            }
            "userVerificationRequired" => Some(CredentialProtection::UserVerificationRequired),
            _ => None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            CredentialProtection::UserVerificationOptional => "userVerificationOptional",
            CredentialProtection::UserVerificationOptionalWithCredentialIdList => {
                "userVerificationOptionalWithCredentialIDList"
            }
            CredentialProtection::UserVerificationRequired => "userVerificationRequired",
        }
    }

    pub(crate) fn level(self) -> u8 {
        match self {
            CredentialProtection::UserVerificationOptional => 1,
            CredentialProtection::UserVerificationOptionalWithCredentialIdList => 2,
            CredentialProtection::UserVerificationRequired => 3,
        }
    }

    pub(crate) fn from_level(level: u64) -> Option<CredentialProtection> {
        match level {
            1 => Some(CredentialProtection::UserVerificationOptional),
            2 => Some(CredentialProtection::UserVerificationOptionalWithCredentialIdList),
            3 => Some(CredentialProtection::UserVerificationRequired),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LargeBlobSupport {
    Required,
    Preferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct CreateInputs {
    pub(crate) cred_protect: Option<(CredentialProtection, bool)>,
    pub(crate) cred_props: bool,
    pub(crate) hmac_create_secret: bool,
    pub(crate) prf: Option<Option<PrfValues>>,
    pub(crate) min_pin_length: bool,
    pub(crate) large_blob: Option<LargeBlobSupport>,
}

impl CreateInputs {
    pub(crate) fn wants_hmac_secret(&self) -> bool {
        self.hmac_create_secret || self.prf.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct CreateOutputs {
    pub(crate) cred_props: Option<bool>,
    pub(crate) cred_protect: Option<CredentialProtection>,
    pub(crate) hmac_create_secret: Option<bool>,
    pub(crate) prf: Option<PrfOutput>,
    pub(crate) min_pin_length: Option<u32>,
    pub(crate) large_blob_supported: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecretUse {
    Prf,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct GetInputs {
    pub(crate) prf: Option<PrfValues>,
    pub(crate) prf_by_credential: Vec<(Vec<u8>, PrfValues)>,
    pub(crate) hmac_secret: Option<Salts>,
    pub(crate) large_blob_read: bool,
}

impl GetInputs {
    pub(crate) fn salts_for(&self, credential: Option<&[u8]>) -> Option<(Salts, SecretUse)> {
        if let Some(salts) = self.hmac_secret {
            return Some((salts, SecretUse::Raw));
        }
        let chosen = credential
            .and_then(|credential| {
                self.prf_by_credential
                    .iter()
                    .find(|(id, _)| id == credential)
                    .map(|(_, values)| values)
            })
            .or(self.prf.as_ref())?;
        Some((chosen.salts(), SecretUse::Prf))
    }
}
