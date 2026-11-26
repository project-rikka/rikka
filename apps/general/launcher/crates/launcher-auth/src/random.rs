use std::fmt::Write as _;

use zeroize::Zeroizing;

use crate::error::{Error, Result};

pub(crate) fn bytes<const N: usize>() -> Result<Zeroizing<[u8; N]>> {
    let mut drawn = Zeroizing::new([0u8; N]);
    getrandom::fill(drawn.as_mut_slice()).map_err(Error::Random)?;
    Ok(drawn)
}

pub(crate) fn uuid() -> Result<String> {
    let mut drawn = *bytes::<16>()?;
    drawn[6] = (drawn[6] & 0x0F) | 0x40;
    drawn[8] = (drawn[8] & 0x3F) | 0x80;
    let mut text = String::with_capacity(36);
    for (index, byte) in drawn.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        write!(text, "{byte:02x}").expect("writing to a string cannot fail");
    }
    Ok(text)
}
