use aes::Aes128;
use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, KeyInit, Mac};
use rand::RngCore;
use sha1::Sha1;
use sha2::Sha256;
use zeroize::Zeroizing;

const KEY: usize = 16;
const BLOCK: usize = 16;
const MAC: usize = 32;
const PBKDF2_SALT: [u8; 16] = [
    0xc7, 0xca, 0xfb, 0x23, 0xec, 0x2a, 0x9d, 0x4c, 0x03, 0x5a, 0x90, 0xae, 0xed, 0x8b, 0xa4, 0x98,
];
const ENCRYPTION_ROUNDS: u32 = 1003;
const MAC_ROUNDS: u32 = 1004;
const SCRYPT_LOG_N: u8 = 13;
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 11;
const PASSWORD_TYPE: u32 = 1;
const KEY_NAME: &[u8] = b"nigori-key";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Derivation {
    Pbkdf2,
    Scrypt { salt: Vec<u8> },
}

#[derive(Clone, PartialEq, Eq)]
pub struct Nigori {
    encryption_key: Zeroizing<[u8; KEY]>,
    mac_key: Zeroizing<[u8; KEY]>,
}

impl Nigori {
    #[must_use]
    pub fn derive(derivation: &Derivation, password: &[u8]) -> Nigori {
        let mut encryption_key = Zeroizing::new([0; KEY]);
        let mut mac_key = Zeroizing::new([0; KEY]);
        match derivation {
            Derivation::Pbkdf2 => {
                pbkdf2_sha1(
                    password,
                    &PBKDF2_SALT,
                    ENCRYPTION_ROUNDS,
                    &mut encryption_key,
                );
                pbkdf2_sha1(password, &PBKDF2_SALT, MAC_ROUNDS, &mut mac_key);
            }
            Derivation::Scrypt { salt } => {
                let params = scrypt::Params::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P)
                    .expect("the sync scrypt parameters are valid");
                let mut keys = Zeroizing::new([0; KEY * 2]);
                scrypt::scrypt(password, salt, &params, keys.as_mut_slice())
                    .expect("scrypt derives 32 bytes");
                encryption_key.copy_from_slice(&keys[..KEY]);
                mac_key.copy_from_slice(&keys[KEY..]);
            }
        }
        Nigori {
            encryption_key,
            mac_key,
        }
    }

    #[must_use]
    pub fn import(encryption_key: &[u8], mac_key: &[u8]) -> Option<Nigori> {
        let encryption_key: [u8; KEY] = encryption_key.try_into().ok()?;
        let mac_key: [u8; KEY] = mac_key.try_into().ok()?;
        Some(Nigori {
            encryption_key: Zeroizing::new(encryption_key),
            mac_key: Zeroizing::new(mac_key),
        })
    }

    #[must_use]
    pub fn encryption_key(&self) -> &[u8] {
        self.encryption_key.as_slice()
    }

    #[must_use]
    pub fn mac_key(&self) -> &[u8] {
        self.mac_key.as_slice()
    }

    #[must_use]
    pub fn name(&self) -> String {
        let mut plain = Vec::with_capacity(8 + 4 + KEY_NAME.len());
        plain.extend_from_slice(&4_u32.to_be_bytes());
        plain.extend_from_slice(&PASSWORD_TYPE.to_be_bytes());
        plain.extend_from_slice(&(KEY_NAME.len() as u32).to_be_bytes());
        plain.extend_from_slice(KEY_NAME);
        let mut named = self.cbc_encrypt(&[0; BLOCK], &plain);
        let mac = self.mac(&[&named]);
        named.extend_from_slice(&mac);
        STANDARD.encode(named)
    }

    #[must_use]
    pub fn encrypt(&self, plain: &[u8]) -> String {
        let mut iv = [0; BLOCK];
        rand::rng().fill_bytes(&mut iv);
        let ciphertext = self.cbc_encrypt(&iv, plain);
        let mac = self.mac(&[&ciphertext]);
        let mut sealed = Vec::with_capacity(BLOCK + ciphertext.len() + MAC);
        sealed.extend_from_slice(&iv);
        sealed.extend_from_slice(&ciphertext);
        sealed.extend_from_slice(&mac);
        STANDARD.encode(sealed)
    }

    #[must_use]
    pub fn decrypt(&self, blob: &str) -> Option<Zeroizing<Vec<u8>>> {
        let sealed = STANDARD.decode(blob).ok()?;
        if sealed.len() < BLOCK * 2 + MAC {
            return None;
        }
        let (body, mac) = sealed.split_at(sealed.len() - MAC);
        let (iv, ciphertext) = body.split_at(BLOCK);
        if !self.verifies(&[body], mac) && !self.verifies(&[ciphertext], mac) {
            return None;
        }
        if !ciphertext.len().is_multiple_of(BLOCK) {
            return None;
        }
        let iv: [u8; BLOCK] = iv.try_into().ok()?;
        let mut buffer = Zeroizing::new(ciphertext.to_vec());
        let opened = cbc::Decryptor::<Aes128>::new(&(*self.encryption_key).into(), &iv.into())
            .decrypt_padded::<Pkcs7>(&mut buffer)
            .ok()?
            .len();
        buffer.truncate(opened);
        Some(buffer)
    }

    fn cbc_encrypt(&self, iv: &[u8; BLOCK], plain: &[u8]) -> Vec<u8> {
        let padded = (plain.len() / BLOCK + 1) * BLOCK;
        let mut buffer = Zeroizing::new(vec![0; padded]);
        buffer[..plain.len()].copy_from_slice(plain);
        cbc::Encryptor::<Aes128>::new(&(*self.encryption_key).into(), &(*iv).into())
            .encrypt_padded::<Pkcs7>(&mut buffer, plain.len())
            .expect("the buffer holds the padding")
            .to_vec()
    }

    fn keyed(&self) -> Hmac<Sha256> {
        <Hmac<Sha256> as KeyInit>::new_from_slice(self.mac_key.as_slice())
            .expect("HMAC accepts any key length")
    }

    fn mac(&self, parts: &[&[u8]]) -> [u8; MAC] {
        let mut keyed = self.keyed();
        for part in parts {
            keyed.update(part);
        }
        keyed.finalize().into_bytes().into()
    }

    fn verifies(&self, parts: &[&[u8]], mac: &[u8]) -> bool {
        let mut keyed = self.keyed();
        for part in parts {
            keyed.update(part);
        }
        keyed.verify_slice(mac).is_ok()
    }
}

impl std::fmt::Debug for Nigori {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Nigori").finish_non_exhaustive()
    }
}

fn pbkdf2_sha1(password: &[u8], salt: &[u8], rounds: u32, derived: &mut [u8; KEY]) {
    let keyed =
        <Hmac<Sha1> as KeyInit>::new_from_slice(password).expect("HMAC accepts any key length");
    let mut first = keyed.clone();
    first.update(salt);
    first.update(&1_u32.to_be_bytes());
    let mut block: [u8; 20] = first.finalize().into_bytes().into();
    let mut folded = block;
    for _ in 1..rounds {
        let mut next = keyed.clone();
        next.update(&block);
        block = next.finalize().into_bytes().into();
        for (folded, byte) in folded.iter_mut().zip(block) {
            *folded ^= byte;
        }
    }
    derived.copy_from_slice(&folded[..KEY]);
}
