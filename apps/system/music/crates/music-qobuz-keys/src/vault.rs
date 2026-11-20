use std::path::Path;

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use music_core::{Result, keep_secret, kept_secret};
use rand::RngCore;

use crate::content::{ContentKey, unreadable};

const KIND: &str = "content-keys";
const LABEL: &str = "Qobuz downloaded track keys";
const VAULT_LEN: usize = 32;
const NONCE: usize = 12;
const SEALING: &[u8] = b"rikka-music qobuz content key";

pub struct Vault {
    cipher: Aes256Gcm,
}

impl Vault {
    pub async fn open(folder: &Path) -> Result<Vault> {
        let key = match kept_secret::<[u8; VAULT_LEN]>(folder, KIND).await? {
            Some(key) => key,
            None => {
                let mut key = [0; VAULT_LEN];
                rand::rng().fill_bytes(&mut key);
                keep_secret(folder, KIND, LABEL, &key).await?;
                key
            }
        };
        Ok(Vault {
            cipher: Aes256Gcm::new(&Key::<Aes256Gcm>::from(key)),
        })
    }

    pub fn seal(&self, key: &ContentKey) -> Result<Vec<u8>> {
        let mut nonce = [0; NONCE];
        rand::rng().fill_bytes(&mut nonce);
        let sealed = self
            .cipher
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: key,
                    aad: SEALING,
                },
            )
            .map_err(|_| unreadable("the track key cannot be sealed"))?;
        Ok([&nonce[..], &sealed].concat())
    }

    pub fn unseal(&self, sealed: &[u8]) -> Result<ContentKey> {
        if sealed.len() <= NONCE {
            return Err(unreadable("cached track key is too short"));
        }
        let (nonce, message) = sealed.split_at(NONCE);
        let nonce =
            Nonce::try_from(nonce).map_err(|_| unreadable("invalid cached track key nonce"))?;
        self.cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: message,
                    aad: SEALING,
                },
            )
            .ok()
            .and_then(|plain| plain.try_into().ok())
            .ok_or_else(|| unreadable("cached track key belongs to another vault"))
    }
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Vault").finish_non_exhaustive()
    }
}
