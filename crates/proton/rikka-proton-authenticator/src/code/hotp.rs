use rikka_proton_pgp::{Hash, hmac};

use crate::code::algorithm::Algorithm;
use crate::error::{Error, Result};

pub(crate) const MAX_DIGITS: u8 = 9;

pub(crate) fn hotp(
    secret: &[u8],
    counter: u64,
    algorithm: Algorithm,
    digits: u8,
) -> Result<String> {
    if digits == 0 || digits > MAX_DIGITS {
        return Err(Error::InvalidDigits {
            digits: digits.to_string(),
        });
    }
    let value = truncate(&counter_hmac(algorithm, secret, counter));
    let modulus = 10u32.pow(u32::from(digits));
    Ok(format!(
        "{:0width$}",
        value % modulus,
        width = usize::from(digits)
    ))
}

pub(crate) fn truncate(mac: &[u8]) -> u32 {
    let offset = usize::from(mac[mac.len() - 1] & 0x0f);
    let chunk = [
        mac[offset],
        mac[offset + 1],
        mac[offset + 2],
        mac[offset + 3],
    ];
    u32::from_be_bytes(chunk) & 0x7fff_ffff
}

pub(crate) fn counter_hmac(algorithm: Algorithm, secret: &[u8], counter: u64) -> Vec<u8> {
    let hash = match algorithm {
        Algorithm::Sha1 => Hash::Sha1,
        Algorithm::Sha256 => Hash::Sha256,
        Algorithm::Sha512 => Hash::Sha512,
    };
    hmac(hash, secret, counter.to_be_bytes())
}
