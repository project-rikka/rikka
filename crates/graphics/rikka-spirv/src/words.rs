use crate::error::{Error, Result};

pub fn words(bytes: &[u8]) -> Result<Vec<u32>> {
    let (words, rest) = bytes.as_chunks::<4>();
    if !rest.is_empty() {
        return Err(Error::Length(bytes.len()));
    }
    Ok(words.iter().map(|word| u32::from_le_bytes(*word)).collect())
}

pub fn bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}
