use crate::content::file::FileMetadata;
use crate::content::folder::Folder;
use crate::content::item::Item;
use crate::content::vault::Vault;
use crate::error::{Error, Result};

macro_rules! contents {
    ($($type:ty => $subject:literal,)*) => {
        $(impl $type {
            pub fn decode(bytes: &[u8]) -> Result<Self> {
                <Self as prost::Message>::decode(bytes)
                    .map_err(|error| Error::malformed($subject, error))
            }

            pub fn encode(&self) -> Vec<u8> {
                prost::Message::encode_to_vec(self)
            }
        })*
    };
}

contents! {
    Item => "item",
    Vault => "vault",
    Folder => "folder",
    FileMetadata => "file",
}
