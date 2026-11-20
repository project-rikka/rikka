use rustls::pki_types::TrustAnchor;

use crate::spki_pin::SpkiPin;

const SEQUENCE: u8 = 0x30;
const LONG_LENGTH: u8 = 0x80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pinning {
    Chain(Vec<SpkiPin>),
    Leaf(Vec<SpkiPin>),
}

pub(crate) fn matches_pin(pins: &[SpkiPin], public_key_info: &[u8]) -> bool {
    pins.contains(&SpkiPin::from_public_key_info(public_key_info))
}

pub(crate) fn anchor_public_key_info(anchor: &TrustAnchor<'_>) -> Vec<u8> {
    let contents = anchor.subject_public_key_info.as_ref();
    let mut encoded = Vec::with_capacity(contents.len() + 6);
    encoded.push(SEQUENCE);
    encoded.extend(length(contents.len()));
    encoded.extend_from_slice(contents);
    encoded
}

fn length(length: usize) -> Vec<u8> {
    if length < usize::from(LONG_LENGTH) {
        return vec![length as u8];
    }
    let bytes = length.to_be_bytes();
    let significant: Vec<u8> = bytes
        .iter()
        .copied()
        .skip_while(|byte| *byte == 0)
        .collect();
    let mut encoded = Vec::with_capacity(significant.len() + 1);
    encoded.push(LONG_LENGTH | significant.len() as u8);
    encoded.extend(significant);
    encoded
}
