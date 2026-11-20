use rikka_proton_pgp::PublicKey;

use crate::client::DriveClient;
use crate::error::Result;

pub(crate) struct PublishedKeys {
    pub(crate) verification: Vec<PublicKey>,
    pub(crate) encryption: Option<PublicKey>,
}

impl DriveClient {
    pub(crate) async fn verifying_keys(&self, email: &str) -> Result<Vec<PublicKey>> {
        match crate::crypto::verification_keys(self.keys(), Some(email)) {
            own if !own.is_empty() => Ok(own),
            _ => Ok(self.published_keys(email).await?.verification),
        }
    }

    pub(crate) async fn published_keys(&self, email: &str) -> Result<PublishedKeys> {
        let published = self
            .account()
            .public_keys(email)
            .send()
            .await?
            .into_public_keys()?;
        let listed = published.address().keys();

        let verification = listed.iter().filter_map(|key| key.key().ok()).collect();
        let encryption = listed
            .iter()
            .filter(|key| key.can_encrypt())
            .find_map(|key| key.key().ok());

        Ok(PublishedKeys {
            verification,
            encryption,
        })
    }
}
