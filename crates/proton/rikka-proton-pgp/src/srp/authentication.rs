use std::fmt;

use proton_srp::{SRPAuth, SRPProofB64};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::srp::hash_version::HashVersion;
use crate::srp::modulus::{Modulus, VerifiedModulus};
use crate::srp::proofs::SrpProofs;

#[must_use]
pub struct Authentication {
    username: Option<String>,
    password: Zeroizing<String>,
    version: HashVersion,
    salt: String,
    modulus: Option<Modulus>,
    server_ephemeral: String,
}

impl Authentication {
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Zeroizing::new(password.into());
        self
    }

    pub fn version(mut self, version: HashVersion) -> Self {
        self.version = version;
        self
    }

    pub fn salt(mut self, salt: impl Into<String>) -> Self {
        self.salt = salt.into();
        self
    }

    pub fn modulus(mut self, modulus: Modulus) -> Self {
        self.modulus = Some(modulus);
        self
    }

    pub fn server_ephemeral(mut self, server_ephemeral: impl Into<String>) -> Self {
        self.server_ephemeral = server_ephemeral.into();
        self
    }

    pub fn proofs(self) -> Result<SrpProofs> {
        let modulus = self.modulus.ok_or(Error::Invalid("missing SRP modulus"))?;
        if self.server_ephemeral.is_empty() {
            return Err(Error::Invalid("missing SRP server ephemeral"));
        }
        if self.version.needs_username() && self.username.is_none() {
            return Err(Error::Invalid("SRP hash version requires a username"));
        }

        let authentication = SRPAuth::new(
            &VerifiedModulus,
            self.username.as_deref(),
            &self.password,
            self.version.into(),
            &self.salt,
            modulus.as_str(),
            &self.server_ephemeral,
        )?;
        let proofs = SRPProofB64::from(authentication.generate_proofs()?);
        Ok(SrpProofs::new(proofs))
    }

    pub(crate) fn new() -> Self {
        Authentication {
            username: None,
            password: Zeroizing::default(),
            version: HashVersion::default(),
            salt: String::new(),
            modulus: None,
            server_ephemeral: String::new(),
        }
    }
}

impl fmt::Debug for Authentication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Authentication")
            .field("username", &self.username)
            .field("version", &self.version)
            .field("salt", &self.salt)
            .field("modulus", &self.modulus)
            .finish_non_exhaustive()
    }
}
