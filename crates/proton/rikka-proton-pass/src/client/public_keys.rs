use rikka_proton_account::{PublishedAddress, PublishedKey};
use rikka_proton_pgp::PublicKey;

use crate::client::PassClient;
use crate::error::{Error, Result};

impl PassClient {
    pub(crate) async fn proton_keys(&self, email: &str) -> Result<Option<Vec<PublishedKey>>> {
        let published = self
            .account()
            .public_keys(email)
            .internal_only()
            .send()
            .await?;

        Ok(match published {
            PublishedAddress::Keys(keys) => Some(keys.address().keys().to_vec()),
            PublishedAddress::Unpublished(_) => None,
        })
    }

    pub(crate) async fn encryption_key(&self, email: &str) -> Result<PublicKey> {
        let keys = self.proton_keys(email).await?.unwrap_or_default();

        select_encryption_key(email, &keys)
    }

    pub(crate) async fn verifying_keys(&self, email: &str) -> Result<Vec<PublicKey>> {
        let keys = self.proton_keys(email).await?.unwrap_or_default();
        let usable = keys
            .iter()
            .filter(|key| key.can_verify())
            .map(PublishedKey::key)
            .collect::<rikka_proton_account::Result<Vec<PublicKey>>>()?;

        if usable.is_empty() {
            return Err(Error::NoPublicKey {
                email: email.to_owned(),
            });
        }
        Ok(usable)
    }
}

pub(crate) fn select_encryption_key(email: &str, keys: &[PublishedKey]) -> Result<PublicKey> {
    let key = keys
        .iter()
        .find(|key| key.can_encrypt())
        .ok_or_else(|| Error::NoPublicKey {
            email: email.to_owned(),
        })?;

    Ok(key.key()?)
}
