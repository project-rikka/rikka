use crate::ShareResponse;
use crate::client::{PREFIX, PassClient};
use crate::content::Vault;
use crate::error::{Error, Result};
use crate::identifier::ShareId;
use crate::new_vault::NewVault;
use crate::share::Share;

impl PassClient {
    pub async fn create_vault(&self, vault: NewVault) -> Result<Share> {
        let NewVault {
            vault: content,
            address,
        } = vault;
        let address = address
            .or_else(|| self.address().cloned())
            .ok_or(rikka_proton_account::Error::NoAddress)?;

        let (request, _) =
            crate::crypto::vault::create_request(self.pgp(), self.keys(), address, &content)?;

        let envelope: ShareResponse = self
            .api()
            .post(format!("{PREFIX}/vault"))
            .json(&request)
            .send_field("Share")
            .await?;

        Ok(Share::new(envelope, Some(content)))
    }

    pub async fn update_vault(&self, share: &ShareId, vault: &Vault) -> Result<Share> {
        let response = self.share_response(share).await?;
        self.ensure_vault(&response)?;

        let rotations: Vec<i64> = response.content_key_rotation().into_iter().collect();
        let key = self
            .share_keys_for_rotations(share, &rotations)
            .await?
            .latest()
            .cloned()
            .ok_or_else(|| Error::NoShareKey {
                share: share.clone(),
            })?;

        let request = crate::crypto::vault::update_request(&key, vault)?;
        let envelope: ShareResponse = self
            .api()
            .put(format!("{PREFIX}/vault/{share}"))
            .json(&request)
            .send_field("Share")
            .await?;

        Ok(Share::new(envelope, Some(vault.clone())))
    }

    pub async fn delete_vault(&self, share: &ShareId) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/vault/{share}"))
            .send_unit()
            .await?;
        self.keyring.remove_share(share);
        self.remove_cached_share(share);

        Ok(())
    }

    pub async fn make_vault_primary(&self, share: &ShareId) -> Result<()> {
        self.api()
            .put(format!("{PREFIX}/vault/{share}/primary"))
            .send_unit()
            .await?;

        Ok(())
    }
}
