use crate::content::custom::CustomSection;
use crate::content::non_empty;

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ItemSshKey {
    #[prost(string, tag = "1")]
    pub private_key: String,
    #[prost(string, tag = "2")]
    pub public_key: String,
    #[prost(message, repeated, tag = "3")]
    pub sections: Vec<CustomSection>,
}

impl ItemSshKey {
    pub fn private(&self) -> Option<&str> {
        non_empty(&self.private_key)
    }

    pub fn public(&self) -> Option<&str> {
        non_empty(&self.public_key)
    }
}
