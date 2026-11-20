use crate::code::algorithm::Algorithm;
use crate::code::hotp::{counter_hmac, truncate};

const ALPHABET: [char; 26] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'B', 'C', 'D', 'F', 'G', 'H', 'J', 'K', 'M', 'N', 'P',
    'Q', 'R', 'T', 'V', 'W', 'X', 'Y',
];

pub(crate) const DIGITS: u8 = 5;
pub(crate) const PERIOD: u16 = 30;
pub(crate) const ISSUER: &str = "Steam";

pub(crate) fn code(secret: &[u8], counter: u64) -> String {
    let mut value = truncate(&counter_hmac(Algorithm::Sha1, secret, counter));
    let mut code = String::with_capacity(usize::from(DIGITS));
    let letters = ALPHABET.len() as u32;
    for _ in 0..DIGITS {
        code.push(ALPHABET[(value % letters) as usize]);
        value /= letters;
    }
    code
}
