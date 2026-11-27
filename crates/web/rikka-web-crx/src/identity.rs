use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::pkcs8::EncodePublicKey;
use sha2::{Digest, Sha256};

const RIKKA_SEED: &[u8] = b"os.rikka.web-extension";

const ID_BYTES: usize = 16;

#[derive(Clone)]
pub struct Identity {
    key: SigningKey,
    public_key: Vec<u8>,
}

impl Identity {
    #[must_use]
    pub fn rikka() -> Identity {
        Identity::from_seed(RIKKA_SEED)
    }

    #[must_use]
    pub fn from_seed(seed: &[u8]) -> Identity {
        let key = (0u32..)
            .find_map(|round| {
                let digest = Sha256::new()
                    .chain_update(seed)
                    .chain_update(round.to_be_bytes())
                    .finalize();
                SigningKey::from_slice(&digest).ok()
            })
            .expect("a derived scalar is valid within a few rounds");
        let public_key = key
            .verifying_key()
            .to_public_key_der()
            .expect("a P-256 public key always encodes")
            .into_vec();
        Identity { key, public_key }
    }

    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    #[must_use]
    pub fn manifest_key(&self) -> String {
        STANDARD.encode(&self.public_key)
    }

    #[must_use]
    pub fn crx_id(&self) -> [u8; ID_BYTES] {
        let digest = Sha256::digest(&self.public_key);
        let mut id = [0; ID_BYTES];
        id.copy_from_slice(&digest[..ID_BYTES]);
        id
    }

    #[must_use]
    pub fn extension_id(&self) -> String {
        self.crx_id()
            .iter()
            .flat_map(|byte| [byte >> 4, byte & 0x0f])
            .map(|nibble| char::from(b'a' + nibble))
            .collect()
    }

    pub(crate) fn sign(&self, message: &[u8]) -> Vec<u8> {
        let signature: Signature = self.key.sign(message);
        signature.to_der().as_bytes().to_vec()
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Identity")
            .field("extension_id", &self.extension_id())
            .finish_non_exhaustive()
    }
}
