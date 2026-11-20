use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

use crate::random;

pub struct Pkce {
    verifier: String,
    challenge: String,
}

impl Pkce {
    #[must_use]
    pub fn new() -> Pkce {
        Pkce::from_verifier(URL_SAFE_NO_PAD.encode(random::bytes::<96>()))
    }

    fn from_verifier(verifier: String) -> Pkce {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Pkce {
            verifier,
            challenge,
        }
    }

    #[must_use]
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    #[must_use]
    pub fn challenge(&self) -> &str {
        &self.challenge
    }

    #[must_use]
    pub fn method(&self) -> &'static str {
        "S256"
    }
}

impl Default for Pkce {
    fn default() -> Pkce {
        Pkce::new()
    }
}

impl std::fmt::Debug for Pkce {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Pkce").finish_non_exhaustive()
    }
}
