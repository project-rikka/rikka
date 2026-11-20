use rikka_proton_pgp::{KeyRing, Pgp, SecretBytes, VerificationContext};

use crate::error::{Error, Result};

const TOKEN_LEN: usize = 64;

#[must_use]
#[derive(Debug, Clone, Copy)]
pub struct KeyToken<'a> {
    token: &'a str,
    signature: &'a str,
    context: Option<&'a str>,
}

impl<'a> KeyToken<'a> {
    pub fn new(token: &'a str, signature: &'a str) -> KeyToken<'a> {
        KeyToken {
            token,
            signature,
            context: None,
        }
    }

    pub fn context(mut self, context: &'a str) -> Self {
        self.context = Some(context);
        self
    }

    pub fn open(&self, pgp: &Pgp, ring: &KeyRing) -> Result<SecretBytes> {
        let decrypted = pgp.decrypt().with_ring(ring).message(self.token)?;
        let secret = SecretBytes::new(decrypted.into_data());

        if secret.len() != TOKEN_LEN || !secret.expose().iter().all(u8::is_ascii_hexdigit) {
            return Err(Error::MalformedKeyToken);
        }

        let mut verification = pgp.verify().with_ring(ring);
        if let Some(context) = self.context {
            verification = verification.with_context(VerificationContext::new(context));
        }
        let status = verification.detached(secret.expose(), self.signature);

        match status.is_valid() {
            true => Ok(secret),
            false => Err(Error::UnsignedKeyToken(Box::new(status))),
        }
    }
}
