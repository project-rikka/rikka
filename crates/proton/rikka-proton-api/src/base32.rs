#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alphabet {
    Rfc4648,
    Crockford,
}

impl Alphabet {
    fn symbols(self) -> &'static [u8; 32] {
        match self {
            Alphabet::Rfc4648 => b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567",
            Alphabet::Crockford => b"0123456789ABCDEFGHJKMNPQRSTVWXYZ",
        }
    }
}

#[must_use]
pub fn encode(bytes: impl AsRef<[u8]>) -> String {
    encode_with(bytes, Alphabet::Rfc4648)
}

#[must_use]
pub fn encode_with(bytes: impl AsRef<[u8]>, alphabet: Alphabet) -> String {
    let symbols = alphabet.symbols();
    let bytes = bytes.as_ref();
    let mut encoded = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let mut buffer = 0u16;
    let mut bits = 0u32;

    for byte in bytes {
        buffer = (buffer << 8) | u16::from(*byte);
        bits += 8;

        while bits >= 5 {
            bits -= 5;
            let index = (buffer >> bits) & 0b1_1111;
            encoded.push(char::from(symbols[usize::from(index)]));
        }
    }

    if bits > 0 {
        let index = (buffer << (5 - bits)) & 0b1_1111;
        encoded.push(char::from(symbols[usize::from(index)]));
    }

    encoded
}
