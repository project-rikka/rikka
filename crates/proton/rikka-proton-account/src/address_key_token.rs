use rikka_proton_pgp::{Pgp, PrivateKey, SecretBytes};

use crate::error::Result;

const TOKEN_LEN: usize = 32;

pub(crate) struct AddressKeyToken {
    pub(crate) secret: SecretBytes,
    pub(crate) encrypted: String,
    pub(crate) signature: String,
}

impl AddressKeyToken {
    pub(crate) fn generate(pgp: &Pgp, user_key: &PrivateKey) -> Result<AddressKeyToken> {
        let secret = SecretBytes::random_hex(TOKEN_LEN);

        let encrypted = pgp
            .encrypt()
            .to(user_key.public_key())
            .as_utf8()
            .armored_message(secret.expose())?;
        let signature = pgp
            .sign()
            .with_key(user_key.clone())
            .as_utf8()
            .armored_detached(secret.expose())?;

        Ok(AddressKeyToken {
            secret,
            encrypted,
            signature,
        })
    }
}
