use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use rsa::pkcs8::EncodePublicKey;
use rsa::rand_core::OsRng;
use rsa::sha2::{Digest, Sha256};
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};

use crate::error::{Error, Result};

const BITS: usize = 2048;

pub(crate) struct Keys {
    private: RsaPrivateKey,
    public: Vec<u8>,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Keys(..)")
    }
}

impl Keys {
    pub(crate) fn generate() -> Result<Keys> {
        Keys::with_bits(BITS)
    }

    fn with_bits(bits: usize) -> Result<Keys> {
        let private = RsaPrivateKey::new(&mut OsRng, bits).map_err(handshake)?;
        let public = RsaPublicKey::from(&private)
            .to_public_key_der()
            .map_err(handshake)?
            .as_bytes()
            .to_vec();
        Ok(Keys { private, public })
    }

    pub(crate) fn encoded_public_key(&self) -> String {
        STANDARD.encode(&self.public)
    }

    pub(crate) fn fingerprint(&self) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(&self.public))
    }

    pub(crate) fn decrypt(&self, encoded: &str) -> Result<Vec<u8>> {
        let sealed = STANDARD.decode(encoded.trim()).map_err(handshake)?;
        self.private
            .decrypt(Oaep::new::<Sha256>(), &sealed)
            .map_err(handshake)
    }

    pub(crate) fn proof(&self, encrypted_nonce: &str) -> Result<String> {
        Ok(URL_SAFE_NO_PAD.encode(self.decrypt(encrypted_nonce)?))
    }
}

fn handshake(error: impl std::fmt::Display) -> Error {
    Error::Handshake(error.to_string())
}
