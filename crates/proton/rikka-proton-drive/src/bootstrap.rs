use rikka_proton_account::{AddressId, KeyId};
use serde::Serialize;

use crate::client::DriveClient;
use crate::crypto;
use crate::error::Result;

pub(crate) struct Bootstrap {
    pub(crate) share: ShareBootstrap,
    pub(crate) root: RootBootstrap,
}

#[derive(Debug, Serialize)]
pub(crate) struct ShareBootstrap {
    #[serde(rename = "AddressID")]
    pub(crate) address_id: AddressId,
    #[serde(rename = "AddressKeyID")]
    pub(crate) address_key_id: KeyId,
    #[serde(rename = "Key")]
    pub(crate) key: String,
    #[serde(rename = "Passphrase")]
    pub(crate) passphrase: String,
    #[serde(rename = "PassphraseSignature")]
    pub(crate) passphrase_signature: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RootBootstrap {
    #[serde(rename = "Name")]
    pub(crate) name: String,
    #[serde(rename = "NodeKey")]
    pub(crate) node_key: String,
    #[serde(rename = "NodePassphrase")]
    pub(crate) node_passphrase: String,
    #[serde(rename = "NodePassphraseSignature")]
    pub(crate) node_passphrase_signature: String,
    #[serde(rename = "NodeHashKey")]
    pub(crate) node_hash_key: String,
}

impl DriveClient {
    pub(crate) fn bootstrap(&self, root_name: &str) -> Result<Bootstrap> {
        let pgp = self.pgp();
        let address_id = self.primary_address()?;
        let (address_key_id, address_key) = crypto::primary_address_key(self.keys(), &address_id)?;

        let share = crypto::generate_key(pgp, &address_key.public_key(), &address_key)?;
        let root = crypto::generate_key(pgp, &share.key.public_key(), &address_key)?;
        let name =
            crypto::encrypt_name(pgp, &share.key.public_key(), None, &address_key, root_name)?;
        let (_, node_hash_key) = crypto::generate_hash_key(pgp, &root.key)?;

        Ok(Bootstrap {
            share: ShareBootstrap {
                address_id,
                address_key_id,
                key: share.armored_key,
                passphrase: share.armored_passphrase,
                passphrase_signature: share.armored_signature,
            },
            root: RootBootstrap {
                name,
                node_key: root.armored_key,
                node_passphrase: root.armored_passphrase,
                node_passphrase_signature: root.armored_signature,
                node_hash_key,
            },
        })
    }
}
