use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

pub(crate) struct SecretText(pub(crate) Zeroizing<String>);

impl Serialize for SecretText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SecretText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<SecretText, D::Error> {
        String::deserialize(deserializer).map(|text| SecretText(Zeroizing::new(text)))
    }
}
