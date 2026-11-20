use crate::error::{Error, Result};

const ARMOR_PREFIX: &str = "-----BEGIN PGP ";

pub fn armor_message(data: impl AsRef<[u8]>) -> Result<String> {
    armored_text(proton_rpgp::armor::armor_message(data)?)
}

pub fn armor_signature(data: impl AsRef<[u8]>) -> Result<String> {
    armored_text(proton_rpgp::armor::armor_signature(data)?)
}

pub fn armor_public_key(data: impl AsRef<[u8]>) -> Result<String> {
    armored_text(proton_rpgp::armor::armor_public_key(data)?)
}

pub fn armor_private_key(data: impl AsRef<[u8]>) -> Result<String> {
    armored_text(proton_rpgp::armor::armor_private_key(data)?)
}

pub fn unarmor(data: impl AsRef<[u8]>) -> Result<Vec<u8>> {
    Ok(proton_rpgp::armor::unarmor(data)?)
}

#[must_use]
pub fn is_armored(data: impl AsRef<[u8]>) -> bool {
    std::str::from_utf8(data.as_ref()).is_ok_and(|text| text.trim_start().starts_with(ARMOR_PREFIX))
}

pub(crate) fn armored_text(armored: Vec<u8>) -> Result<String> {
    String::from_utf8(armored).map_err(|error| Error::Malformed(Box::new(error)))
}
