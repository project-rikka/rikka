use std::fmt;

use ed25519_dalek::SigningKey;

use pkcs8::PrivateKeyInfoRef;
use pkcs8::pkcs5::pbes2;

use crate::error::{Error, Result};
use crate::key::encoding::{self, ED25519_PKCS8_PREFIX, ED25519_SPKI_PREFIX, KEY_LENGTH};
use crate::key::x25519::{X25519PublicKey, X25519SecretKey};

const PUBLIC_DER_LENGTH: usize = ED25519_SPKI_PREFIX.len() + KEY_LENGTH;
const PRIVATE_DER_LENGTH: usize = ED25519_PKCS8_PREFIX.len() + KEY_LENGTH;
const PBKDF2_ITERATIONS: u32 = 2048;
const SALT_LENGTH: usize = 16;

const ENCRYPTED_LABEL: &str = "ENCRYPTED PRIVATE KEY";
const PUBLIC_LABEL: &str = "PUBLIC KEY";
const PRIVATE_LABEL: &str = "PRIVATE KEY";

#[derive(Clone)]
pub struct KeyPair {
    signing: SigningKey,
}

impl KeyPair {
    #[must_use]
    pub fn generate() -> KeyPair {
        KeyPair::from_seed(rikka_proton_pgp::random_bytes())
    }

    #[must_use]
    pub fn from_seed(seed: [u8; KEY_LENGTH]) -> KeyPair {
        KeyPair {
            signing: SigningKey::from_bytes(&seed),
        }
    }

    pub fn from_seed_base64(text: &str) -> Result<KeyPair> {
        Ok(KeyPair::from_seed(encoding::decode_key(
            "ed25519 seed",
            text,
        )?))
    }

    #[must_use]
    pub fn seed(&self) -> [u8; KEY_LENGTH] {
        self.signing.to_bytes()
    }

    #[must_use]
    pub fn seed_base64(&self) -> String {
        encoding::encode(&self.seed())
    }

    #[must_use]
    pub fn public_key(&self) -> [u8; KEY_LENGTH] {
        self.signing.verifying_key().to_bytes()
    }

    #[must_use]
    pub fn public_key_base64(&self) -> String {
        encoding::encode(&self.public_key())
    }

    #[must_use]
    pub fn public_key_der(&self) -> [u8; PUBLIC_DER_LENGTH] {
        let mut der = [0_u8; PUBLIC_DER_LENGTH];
        der[..ED25519_SPKI_PREFIX.len()].copy_from_slice(&ED25519_SPKI_PREFIX);
        der[ED25519_SPKI_PREFIX.len()..].copy_from_slice(&self.public_key());
        der
    }

    #[must_use]
    pub fn public_key_der_base64(&self) -> String {
        encoding::encode(&self.public_key_der())
    }

    #[must_use]
    pub fn public_key_pem(&self) -> String {
        encoding::pem(PUBLIC_LABEL, &self.public_key_der())
    }

    #[must_use]
    pub fn private_key_der(&self) -> [u8; PRIVATE_DER_LENGTH] {
        let mut der = [0_u8; PRIVATE_DER_LENGTH];
        der[..ED25519_PKCS8_PREFIX.len()].copy_from_slice(&ED25519_PKCS8_PREFIX);
        der[ED25519_PKCS8_PREFIX.len()..].copy_from_slice(&self.seed());
        der
    }

    #[must_use]
    pub fn private_key_der_base64(&self) -> String {
        encoding::encode(&self.private_key_der())
    }

    #[must_use]
    pub fn private_key_pem(&self) -> String {
        encoding::pem(PRIVATE_LABEL, &self.private_key_der())
    }

    pub fn encrypted_private_key_pem(&self, passphrase: &str) -> Result<String> {
        let encryption_error = |error: pkcs8::Error| Error::KeyEncryption {
            reason: error.to_string(),
        };
        let parameters = pbes2::Parameters::generate_pbkdf2_sha256_aes256cbc(
            PBKDF2_ITERATIONS,
            &rikka_proton_pgp::random_bytes::<SALT_LENGTH>(),
            rikka_proton_pgp::random_bytes(),
        )
        .map_err(|error| encryption_error(error.into()))?;
        let der = self.private_key_der();
        let encrypted = PrivateKeyInfoRef::try_from(der.as_slice())
            .map_err(encryption_error)?
            .encrypt_with_params(parameters, passphrase)
            .map_err(encryption_error)?;

        Ok(encoding::pem(ENCRYPTED_LABEL, encrypted.as_bytes()))
    }

    #[must_use]
    pub fn x25519_secret_key(&self) -> X25519SecretKey {
        let hash = rikka_proton_pgp::sha512(self.seed());
        let mut secret = [0_u8; KEY_LENGTH];
        secret.copy_from_slice(&hash[..KEY_LENGTH]);

        secret[0] &= 0xF8;
        secret[KEY_LENGTH - 1] &= 0x7F;
        secret[KEY_LENGTH - 1] |= 0x40;

        X25519SecretKey::from_bytes(secret)
    }

    #[must_use]
    pub fn x25519_public_key(&self) -> X25519PublicKey {
        self.x25519_secret_key().public_key()
    }

    #[must_use]
    pub fn fingerprint(&self) -> String {
        self.x25519_public_key().fingerprint()
    }
}

impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyPair")
            .field("public_key", &self.public_key_base64())
            .finish_non_exhaustive()
    }
}
