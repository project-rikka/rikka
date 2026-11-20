use rikka_proton_account::UserKeys;
use rikka_proton_pgp::Pgp;

use crate::crypto::key::AuthenticatorKey;
use crate::error::{Error, Result};

impl AuthenticatorKey {
    pub fn wrap(&self, pgp: &Pgp, keys: &UserKeys) -> Result<String> {
        let message = pgp
            .encrypt()
            .to(keys.signer().public_key())
            .signed_by(keys.signer().clone())
            .message(self.as_bytes())?;

        Ok(rikka_proton_api::base64::encode(message))
    }

    pub fn unwrap(pgp: &Pgp, keys: &UserKeys, wrapped: &str) -> Result<AuthenticatorKey> {
        let message = rikka_proton_api::base64::decode(wrapped).map_err(|_| Error::MalformedKey)?;

        let plaintext = pgp
            .decrypt()
            .with_ring(keys.ring())
            .verified_by_ring(keys.ring())
            .message(&message)?;

        if !plaintext.signature().is_valid() {
            return Err(Error::InvalidKeySignature {
                status: Box::new(plaintext.signature().clone()),
            });
        }

        AuthenticatorKey::from_bytes(plaintext.data())
    }
}
