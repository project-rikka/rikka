use base64::prelude::{BASE64_STANDARD, Engine as _};
use proton_srp::{ModulusSignatureVerifier, ModulusVerifyError};

use crate::error::{Error, Result};
use crate::handle::Pgp;
use crate::public_key::PublicKey;

const MODULUS_BYTES: usize = 256;

const PROTON_SRP_SERVER_KEY: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----

xjMEXAHLgxYJKwYBBAHaRw8BAQdAFurWXXwjTemqjD7CXjXVyKf0of7n9Ctm
L8v9enkzggHNEnByb3RvbkBzcnAubW9kdWx1c8J3BBAWCgApBQJcAcuDBgsJ
BwgDAgkQNQWFxOlRjyYEFQgKAgMWAgECGQECGwMCHgEAAPGRAP9sauJsW12U
MnTQUZpsbJb53d0Wv55mZIIiJL2XulpWPQD/V6NglBd96lZKBmInSXX/kXat
Sv+y0io+LR8i2+jV+AbOOARcAcuDEgorBgEEAZdVAQUBAQdAeJHUz1c9+KfE
kSIgcBRE3WuXC4oj5a2/U3oASExGDW4DAQgHwmEEGBYIABMFAlwBy4MJEDUF
hcTpUY8mAhsMAAD/XQD8DxNI6E78meodQI+wLsrKLeHn32iLvUqJbVDhfWSU
WO4BAMcm1u02t4VKw++ttECPt+HUgPUq5pqQWe5Q2cW4TMsE
=Y4Mw
-----END PGP PUBLIC KEY BLOCK-----";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modulus {
    value: String,
    id: Option<String>,
}

impl Modulus {
    pub fn from_signed(signed_modulus: impl AsRef<str>) -> Result<Self> {
        let key = PublicKey::import(PROTON_SRP_SERVER_KEY.as_bytes())?;
        let verified = Pgp::new()
            .verify()
            .with_key(key)
            .cleartext(signed_modulus.as_ref())?;
        let value = verified
            .into_verified()
            .map_err(|error| Error::InvalidModulus(Box::new(error)))?;
        let value =
            String::from_utf8(value).map_err(|error| Error::InvalidModulus(Box::new(error)))?;
        Modulus::from_base64(value.trim())
    }

    fn from_base64(value: &str) -> Result<Self> {
        let decoded = BASE64_STANDARD
            .decode(value)
            .map_err(|error| Error::InvalidModulus(Box::new(error)))?;
        if decoded.len() != MODULUS_BYTES {
            return Err(Error::Invalid("SRP modulus must be 256 bytes"));
        }
        Ok(Modulus {
            value: value.to_owned(),
            id: None,
        })
    }

    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
}

pub(crate) struct VerifiedModulus;

impl ModulusSignatureVerifier for VerifiedModulus {
    fn verify_and_extract_modulus(
        &self,
        modulus: &str,
        _server_key: &str,
    ) -> std::result::Result<String, ModulusVerifyError> {
        Ok(modulus.to_owned())
    }
}
