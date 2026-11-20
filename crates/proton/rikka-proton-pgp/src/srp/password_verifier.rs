use crate::srp::hash_version::HashVersion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordVerifier {
    version: HashVersion,
    modulus_id: Option<String>,
    salt: String,
    verifier: String,
}

impl PasswordVerifier {
    #[must_use]
    pub fn version(&self) -> HashVersion {
        self.version
    }

    #[must_use]
    pub fn modulus_id(&self) -> Option<&str> {
        self.modulus_id.as_deref()
    }

    #[must_use]
    pub fn salt(&self) -> &str {
        &self.salt
    }

    #[must_use]
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    pub(crate) fn new(
        version: HashVersion,
        modulus_id: Option<String>,
        salt: String,
        verifier: String,
    ) -> Self {
        PasswordVerifier {
            version,
            modulus_id,
            salt,
            verifier,
        }
    }
}
