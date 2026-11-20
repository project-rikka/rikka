use crypto_bigint::modular::{FixedMontyForm, FixedMontyParams};
use crypto_bigint::{Odd, U768};
use rand::CryptoRng;

use crate::error::{Error, Result};

const GROUP_BYTES: usize = 96;
const EXPONENT_BYTES: usize = 95;
const GENERATOR: u8 = 2;
const PRIME: [u8; GROUP_BYTES] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xc9, 0x0f, 0xda, 0xa2, 0x21, 0x68, 0xc2, 0x34,
    0xc4, 0xc6, 0x62, 0x8b, 0x80, 0xdc, 0x1c, 0xd1, 0x29, 0x02, 0x4e, 0x08, 0x8a, 0x67, 0xcc, 0x74,
    0x02, 0x0b, 0xbe, 0xa6, 0x3b, 0x13, 0x9b, 0x22, 0x51, 0x4a, 0x08, 0x79, 0x8e, 0x34, 0x04, 0xdd,
    0xef, 0x95, 0x19, 0xb3, 0xcd, 0x3a, 0x43, 0x1b, 0x30, 0x2b, 0x0a, 0x6d, 0xf2, 0x5f, 0x14, 0x37,
    0x4f, 0xe1, 0x35, 0x6d, 0x6d, 0x51, 0xc2, 0x45, 0xe4, 0x85, 0xb5, 0x76, 0x62, 0x5e, 0x7e, 0xc6,
    0xf4, 0x4c, 0x42, 0xe9, 0xa6, 0x3a, 0x36, 0x20, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];

pub(crate) struct DhSecret {
    exponent: U768,
}

impl DhSecret {
    pub(crate) fn generate<R: CryptoRng + ?Sized>(rng: &mut R) -> DhSecret {
        let mut drawn = [0u8; GROUP_BYTES];
        rng.fill_bytes(&mut drawn[..EXPONENT_BYTES]);
        DhSecret {
            exponent: U768::from_le_slice(&drawn),
        }
    }

    pub(crate) fn public_key(&self) -> Vec<u8> {
        minimal(power(&U768::from_u8(GENERATOR), &self.exponent))
    }

    pub(crate) fn shared_secret(&self, remote: &[u8]) -> Result<Vec<u8>> {
        let remote = element(remote)?;
        Ok(minimal(power(&remote, &self.exponent)))
    }
}

fn prime() -> U768 {
    U768::from_be_slice(&PRIME)
}

fn power(base: &U768, exponent: &U768) -> U768 {
    let modulus = Odd::new(prime())
        .into_option()
        .expect("the Oakley group prime is odd");
    let params = FixedMontyParams::new_vartime(modulus);
    FixedMontyForm::new(base, &params).pow(exponent).retrieve()
}

fn element(bytes: &[u8]) -> Result<U768> {
    let significant = bytes
        .iter()
        .position(|byte| *byte != 0)
        .map_or(&[][..], |first| &bytes[first..]);
    if significant.len() > GROUP_BYTES {
        return Err(Error::Malformed("access point key"));
    }
    let mut padded = [0u8; GROUP_BYTES];
    padded[GROUP_BYTES - significant.len()..].copy_from_slice(significant);
    let element = U768::from_be_slice(&padded);
    if element <= U768::ONE || element >= prime().wrapping_sub(&U768::ONE) {
        return Err(Error::Malformed("access point key"));
    }
    Ok(element)
}

fn minimal(value: U768) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let bytes = bytes.as_ref();
    let first = bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len());
    bytes[first..].to_vec()
}
