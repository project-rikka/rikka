use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Deserializer, Serializer};
use zeroize::Zeroizing;

pub(crate) fn serialize<S: Serializer>(
    bytes: &Zeroizing<Vec<u8>>,
    into: S,
) -> Result<S::Ok, S::Error> {
    into.serialize_str(&Zeroizing::new(STANDARD.encode(bytes.as_slice())))
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    from: D,
) -> Result<Zeroizing<Vec<u8>>, D::Error> {
    let encoded = Zeroizing::new(String::deserialize(from)?);
    STANDARD
        .decode(encoded.as_bytes())
        .map(Zeroizing::new)
        .map_err(serde::de::Error::custom)
}
