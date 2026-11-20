use aes::Aes128;
use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use crypto_bigint::modular::{FixedMontyForm, FixedMontyParams};
use crypto_bigint::{Odd, U1024};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

use crate::error::{Error, Result};

const PLAIN: &str = "plain";
const DH: &str = "dh-ietf1024-sha256-aes128-cbc-pkcs7";

const PRIME: [u8; GROUP_BYTES] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC9, 0x0F, 0xDA, 0xA2, 0x21, 0x68, 0xC2, 0x34,
    0xC4, 0xC6, 0x62, 0x8B, 0x80, 0xDC, 0x1C, 0xD1, 0x29, 0x02, 0x4E, 0x08, 0x8A, 0x67, 0xCC, 0x74,
    0x02, 0x0B, 0xBE, 0xA6, 0x3B, 0x13, 0x9B, 0x22, 0x51, 0x4A, 0x08, 0x79, 0x8E, 0x34, 0x04, 0xDD,
    0xEF, 0x95, 0x19, 0xB3, 0xCD, 0x3A, 0x43, 0x1B, 0x30, 0x2B, 0x0A, 0x6D, 0xF2, 0x5F, 0x14, 0x37,
    0x4F, 0xE1, 0x35, 0x6D, 0x6D, 0x51, 0xC2, 0x45, 0xE4, 0x85, 0xB5, 0x76, 0x62, 0x5E, 0x7E, 0xC6,
    0xF4, 0x4C, 0x42, 0xE9, 0xA6, 0x37, 0xED, 0x6B, 0x0B, 0xFF, 0x5C, 0xB6, 0xF4, 0x06, 0xB7, 0xED,
    0xEE, 0x38, 0x6B, 0xFB, 0x5A, 0x89, 0x9F, 0xA5, 0xAE, 0x9F, 0x24, 0x11, 0x7C, 0x4B, 0x1F, 0xE6,
    0x49, 0x28, 0x66, 0x51, 0xEC, 0xE6, 0x53, 0x81, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];
const GENERATOR: u8 = 2;
const GROUP_BYTES: usize = 128;
const PRIVATE_TOP_MASK: u8 = 0x7F;
const SESSION_KEY_BYTES: usize = 16;
const BLOCK_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Algorithm {
    Plain,
    Dh,
}

impl Algorithm {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Algorithm> {
        match name {
            PLAIN => Some(Algorithm::Plain),
            DH => Some(Algorithm::Dh),
            _ => None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Plain => PLAIN,
            Algorithm::Dh => DH,
        }
    }
}

pub struct KeyExchange {
    private: U1024,
    public: [u8; GROUP_BYTES],
}

impl KeyExchange {
    pub fn new() -> Result<KeyExchange> {
        let mut drawn = Zeroizing::new([0u8; GROUP_BYTES]);
        let private = loop {
            getrandom::fill(drawn.as_mut_slice()).map_err(Error::Random)?;
            drawn[0] &= PRIVATE_TOP_MASK;
            let private = U1024::from_be_slice(drawn.as_slice());
            if private > U1024::ONE {
                break private;
            }
        };
        let mut public = [0u8; GROUP_BYTES];
        public.copy_from_slice(
            power(&U1024::from_u8(GENERATOR), &private)
                .to_be_bytes()
                .as_ref(),
        );
        Ok(KeyExchange { private, public })
    }

    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public
    }

    pub fn agree(&self, peer: &[u8]) -> Result<SessionKey> {
        let peer = group_element(peer)?;
        let mut shared = Zeroizing::new([0u8; GROUP_BYTES]);
        shared.copy_from_slice(power(&peer, &self.private).to_be_bytes().as_ref());
        let mut key = Zeroizing::new([0u8; SESSION_KEY_BYTES]);
        Hkdf::<Sha256>::new(None, shared.as_ref())
            .expand(&[], key.as_mut_slice())
            .map_err(|_| Error::Derivation)?;
        Ok(SessionKey(key))
    }
}

impl Drop for KeyExchange {
    fn drop(&mut self) {
        self.private.zeroize();
    }
}

impl std::fmt::Debug for KeyExchange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeyExchange")
            .finish_non_exhaustive()
    }
}

pub struct SessionKey(Zeroizing<[u8; SESSION_KEY_BYTES]>);

impl SessionKey {
    pub fn seal(&self, plain: &[u8]) -> Result<Sealed> {
        let mut iv = [0u8; BLOCK_BYTES];
        getrandom::fill(&mut iv).map_err(Error::Random)?;
        let padded = (plain.len() / BLOCK_BYTES + 1) * BLOCK_BYTES;
        let mut buffer = Zeroizing::new(vec![0u8; padded]);
        buffer[..plain.len()].copy_from_slice(plain);
        let sealed = cbc::Encryptor::<Aes128>::new(&(*self.0).into(), &iv.into())
            .encrypt_padded::<Pkcs7>(&mut buffer, plain.len())
            .map_err(|_| Error::Padding)?
            .to_vec();
        Ok(Sealed {
            parameters: iv.to_vec(),
            value: sealed,
        })
    }

    pub fn open(&self, parameters: &[u8], value: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let iv: [u8; BLOCK_BYTES] = parameters.try_into().map_err(|_| Error::Parameters)?;
        if value.is_empty() || !value.len().is_multiple_of(BLOCK_BYTES) {
            return Err(Error::Padding);
        }
        let mut buffer = Zeroizing::new(value.to_vec());
        let opened = cbc::Decryptor::<Aes128>::new(&(*self.0).into(), &iv.into())
            .decrypt_padded::<Pkcs7>(&mut buffer)
            .map_err(|_| Error::Padding)?
            .len();
        buffer.truncate(opened);
        Ok(buffer)
    }
}

impl std::fmt::Debug for SessionKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("SessionKey").finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Sealed {
    pub parameters: Vec<u8>,
    pub value: Vec<u8>,
}

impl std::fmt::Debug for Sealed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Sealed").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum Transport {
    Plain,
    Aes(SessionKey),
}

impl Transport {
    #[must_use]
    pub fn algorithm(&self) -> Algorithm {
        match self {
            Transport::Plain => Algorithm::Plain,
            Transport::Aes(_) => Algorithm::Dh,
        }
    }

    pub fn seal(&self, plain: &[u8]) -> Result<Sealed> {
        match self {
            Transport::Plain => Ok(Sealed {
                parameters: Vec::new(),
                value: plain.to_vec(),
            }),
            Transport::Aes(key) => key.seal(plain),
        }
    }

    pub fn open(&self, parameters: &[u8], value: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        match self {
            Transport::Plain => Ok(Zeroizing::new(value.to_vec())),
            Transport::Aes(key) => key.open(parameters, value),
        }
    }
}

fn prime() -> U1024 {
    U1024::from_be_slice(&PRIME)
}

fn power(base: &U1024, exponent: &U1024) -> U1024 {
    let modulus = Odd::new(prime())
        .into_option()
        .expect("the IETF group prime is odd");
    let params = FixedMontyParams::new_vartime(modulus);
    FixedMontyForm::new(base, &params).pow(exponent).retrieve()
}

fn group_element(bytes: &[u8]) -> Result<U1024> {
    let significant = bytes
        .iter()
        .position(|byte| *byte != 0)
        .map_or(&[][..], |first| &bytes[first..]);
    if significant.len() > GROUP_BYTES {
        return Err(Error::Peer);
    }
    let mut padded = [0u8; GROUP_BYTES];
    padded[GROUP_BYTES - significant.len()..].copy_from_slice(significant);
    let element = U1024::from_be_slice(&padded);
    let ceiling = prime().wrapping_sub(&U1024::ONE);
    if element <= U1024::ONE || element >= ceiling {
        return Err(Error::Peer);
    }
    Ok(element)
}
