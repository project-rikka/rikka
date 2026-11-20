use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::srp::Srp;
use crate::srp::hash_version::HashVersion;
use crate::srp::modulus::Modulus;
use crate::srp::proofs::SrpProofs;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SrpChallenge {
    #[serde(rename = "Modulus", default, skip_serializing_if = "Option::is_none")]
    modulus: Option<String>,
    #[serde(
        rename = "ServerEphemeral",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    server_ephemeral: Option<String>,
    #[serde(rename = "Version", default, skip_serializing_if = "Option::is_none")]
    version: Option<u8>,
    #[serde(
        rename = "Salt",
        alias = "SrpSalt",
        alias = "UrlPasswordSalt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    salt: Option<String>,
    #[serde(
        rename = "SRPSession",
        alias = "SrpSessionID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    session: Option<String>,
}

impl SrpChallenge {
    #[must_use]
    pub fn modulus(&self) -> Option<&str> {
        self.modulus.as_deref()
    }

    #[must_use]
    pub fn server_ephemeral(&self) -> Option<&str> {
        self.server_ephemeral.as_deref()
    }

    #[must_use]
    pub fn version(&self) -> Option<u8> {
        self.version
    }

    #[must_use]
    pub fn salt(&self) -> Option<&str> {
        self.salt.as_deref()
    }

    #[must_use]
    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    pub fn proofs(&self, username: Option<&str>, password: &str) -> Result<SrpProofs> {
        let modulus = self
            .modulus
            .as_deref()
            .ok_or(Error::Invalid("SRP challenge has no modulus"))?;
        let version = self
            .version
            .ok_or(Error::Invalid("SRP challenge has no version"))?;
        let salt = self
            .salt
            .as_deref()
            .ok_or(Error::Invalid("SRP challenge has no salt"))?;
        let server_ephemeral = self
            .server_ephemeral
            .as_deref()
            .ok_or(Error::Invalid("SRP challenge has no server ephemeral"))?;

        let mut authentication = Srp::new()
            .authenticate()
            .password(password)
            .version(HashVersion::try_from(version)?)
            .salt(salt)
            .modulus(Modulus::from_signed(modulus)?)
            .server_ephemeral(server_ephemeral);
        if let Some(username) = username {
            authentication = authentication.username(username);
        }
        authentication.proofs()
    }
}
