use std::fmt;

use proton_srp::{SRPAuth, SRPVerifierB64};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::srp::modulus::{Modulus, VerifiedModulus};
use crate::srp::password_verifier::PasswordVerifier;

#[must_use]
pub struct Registration {
    password: Zeroizing<String>,
    salt: Option<String>,
    modulus: Option<Modulus>,
}

impl Registration {
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Zeroizing::new(password.into());
        self
    }

    pub fn salt(mut self, salt: impl Into<String>) -> Self {
        self.salt = Some(salt.into());
        self
    }

    pub fn modulus(mut self, modulus: Modulus) -> Self {
        self.modulus = Some(modulus);
        self
    }

    pub fn verifier(self) -> Result<PasswordVerifier> {
        let modulus = self.modulus.ok_or(Error::Invalid("missing SRP modulus"))?;
        let verifier = SRPAuth::generate_verifier(
            &VerifiedModulus,
            &self.password,
            self.salt.as_deref(),
            modulus.as_str(),
        )?;
        let verifier = SRPVerifierB64::from(verifier);
        Ok(PasswordVerifier::new(
            verifier.version.into(),
            modulus.id().map(str::to_owned),
            verifier.salt,
            verifier.verifier,
        ))
    }

    pub(crate) fn new() -> Self {
        Registration {
            password: Zeroizing::default(),
            salt: None,
            modulus: None,
        }
    }
}

impl fmt::Debug for Registration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Registration")
            .field("salt", &self.salt)
            .field("modulus", &self.modulus)
            .finish_non_exhaustive()
    }
}
