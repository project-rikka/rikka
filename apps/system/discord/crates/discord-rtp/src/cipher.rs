use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};

pub const KEY_LEN: usize = 32;

const COUNTER_LEN: usize = 4;

const TAG_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Aes256Gcm,
    XChaCha20Poly1305,
}

impl Mode {
    const PREFERENCE: [Mode; 2] = [Mode::Aes256Gcm, Mode::XChaCha20Poly1305];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Mode::Aes256Gcm => "aead_aes256_gcm_rtpsize",
            Mode::XChaCha20Poly1305 => "aead_xchacha20_poly1305_rtpsize",
        }
    }

    #[must_use]
    pub fn named(name: &str) -> Option<Mode> {
        Mode::PREFERENCE
            .into_iter()
            .find(|mode| mode.name() == name)
    }

    #[must_use]
    pub fn choose(offered: &[String]) -> Option<Mode> {
        Mode::PREFERENCE
            .into_iter()
            .find(|mode| offered.iter().any(|name| name == mode.name()))
    }
}

pub struct Cipher(Aead256);

enum Aead256 {
    Aes(Box<Aes256Gcm>),
    XChaCha(Box<XChaCha20Poly1305>),
}

impl Cipher {
    #[must_use]
    pub fn new(mode: Mode, key: &[u8; KEY_LEN]) -> Cipher {
        Cipher(match mode {
            Mode::Aes256Gcm => Aead256::Aes(Box::new(Aes256Gcm::new(&(*key).into()))),
            Mode::XChaCha20Poly1305 => {
                Aead256::XChaCha(Box::new(XChaCha20Poly1305::new(&(*key).into())))
            }
        })
    }

    pub(crate) fn encrypt(&self, aad: &[u8], plain: &[u8], counter: u32) -> Option<Vec<u8>> {
        let suffix = counter.to_be_bytes();
        let payload = Payload { msg: plain, aad };
        let mut sealed = match &self.0 {
            Aead256::Aes(cipher) => cipher.encrypt(&nonce::<12>(suffix).into(), payload),
            Aead256::XChaCha(cipher) => cipher.encrypt(&nonce::<24>(suffix).into(), payload),
        }
        .ok()?;
        sealed.extend_from_slice(&suffix);
        Some(sealed)
    }

    pub(crate) fn decrypt(&self, aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
        if sealed.len() < TAG_LEN + COUNTER_LEN {
            return None;
        }
        let (body, suffix) = sealed.split_at(sealed.len() - COUNTER_LEN);
        let suffix: [u8; COUNTER_LEN] = suffix.try_into().ok()?;
        let payload = Payload { msg: body, aad };
        match &self.0 {
            Aead256::Aes(cipher) => cipher.decrypt(&nonce::<12>(suffix).into(), payload),
            Aead256::XChaCha(cipher) => cipher.decrypt(&nonce::<24>(suffix).into(), payload),
        }
        .ok()
    }
}

fn nonce<const LEN: usize>(counter: [u8; COUNTER_LEN]) -> [u8; LEN] {
    let mut nonce = [0; LEN];
    nonce[..COUNTER_LEN].copy_from_slice(&counter);
    nonce
}

impl std::fmt::Debug for Cipher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match self.0 {
            Aead256::Aes(_) => Mode::Aes256Gcm,
            Aead256::XChaCha(_) => Mode::XChaCha20Poly1305,
        };
        formatter.debug_tuple("Cipher").field(&mode).finish()
    }
}
