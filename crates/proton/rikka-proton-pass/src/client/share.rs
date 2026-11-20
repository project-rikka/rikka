use std::sync::Arc;

use futures_util::future::try_join_all;
use rikka_proton_api::{Page, Paging};

use crate::api::{ShareKeyResponse, ShareResponse};
use crate::client::{PAGE_SIZE, PREFIX, PassClient};
use crate::crypto::{ShareKey, ShareKeys};
use crate::error::{Error, Result};
use crate::identifier::{GroupId, ShareId};
use crate::share::Share;

impl PassClient {
    pub async fn shares(&self) -> Result<Vec<Share>> {
        let responses = self.share_responses().await?;
        let decryptions = responses
            .into_iter()
            .map(|response| self.decrypt_share(response));

        try_join_all(decryptions).await
    }

    pub async fn share(&self, share: &ShareId) -> Result<Share> {
        let response = self.share_response(share).await?;
        self.decrypt_share(response).await
    }

    pub async fn share_keys(&self, share: &ShareId) -> Result<ShareKeys> {
        self.share_keys_for_rotations(share, &[]).await
    }

    pub async fn share_key(&self, share: &ShareId, rotation: i64) -> Result<Option<ShareKey>> {
        Ok(self
            .share_keys_for_rotations(share, &[rotation])
            .await?
            .get(rotation)
            .cloned())
    }

    pub async fn leave_share(&self, share: &ShareId) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/share/{share}"))
            .send_unit()
            .await?;
        self.keyring.remove_share(share);
        self.remove_cached_share(share);

        Ok(())
    }

    pub(crate) async fn share_keys_for_rotations(
        &self,
        share: &ShareId,
        rotations: &[i64],
    ) -> Result<ShareKeys> {
        let mut ring = self.keyring.ring(share).await;
        if let Some(keys) = ring.keys_for_rotations(rotations) {
            return Ok(keys.clone());
        }

        let cached = self.stored_share_keys(share, rotations);
        let keys = self.decrypt_share_keys(share, cached).await?;
        ring.set_keys(keys.clone());
        Ok(keys)
    }

    pub(crate) async fn latest_share_key(&self, share: &ShareId) -> Result<ShareKey> {
        let response = self.share_response(share).await?;
        let keys = match response.content_key_rotation() {
            Some(rotation) => self.share_keys_for_rotations(share, &[rotation]).await?,
            None => self.refresh_share_keys(share).await?,
        };

        keys.latest().cloned().ok_or_else(|| Error::NoShareKey {
            share: share.clone(),
        })
    }

    pub(crate) async fn refresh_share_keys(&self, share: &ShareId) -> Result<ShareKeys> {
        let mut ring = self.keyring.ring(share).await;
        let keys = self.decrypt_share_keys(share, None).await?;
        ring.set_keys(keys.clone());
        Ok(keys)
    }

    pub(crate) async fn share_responses(&self) -> Result<Vec<ShareResponse>> {
        let envelope: Vec<ShareResponse> = self
            .api()
            .get(format!("{PREFIX}/share"))
            .send_field_or_default("Shares")
            .await?;
        for share in &envelope {
            self.keyring
                .set_share_group(share.share_id(), share.group());
        }
        self.cache_shares(&envelope);

        Ok(envelope)
    }

    pub(crate) async fn share_response(&self, share: &ShareId) -> Result<ShareResponse> {
        let envelope: ShareResponse = self
            .api()
            .get(format!("{PREFIX}/share/{share}"))
            .send_field("Share")
            .await?;
        self.keyring
            .set_share_group(envelope.share_id(), envelope.group());
        self.cache_share(&envelope);

        Ok(envelope)
    }

    pub(crate) async fn decrypt_share(&self, response: ShareResponse) -> Result<Share> {
        self.keyring
            .set_share_group(response.share_id(), response.group());

        let (Some(content), Some(rotation)) = (response.content(), response.content_key_rotation())
        else {
            return Ok(Share::new(response, None));
        };

        let Some(key) = self
            .share_keys_for_rotations(response.share_id(), &[rotation])
            .await?
            .get(rotation)
            .cloned()
        else {
            return Ok(Share::new(response, None));
        };

        let vault = crate::crypto::vault::decrypt_content(&key, content)?;
        Ok(Share::new(response, Some(vault)))
    }

    async fn decrypt_share_keys(
        &self,
        share: &ShareId,
        cached: Option<Vec<ShareKeyResponse>>,
    ) -> Result<ShareKeys> {
        let group = self.share_group(share).await?;
        let responses = match cached {
            Some(cached) => cached,
            None => self.share_key_responses(share).await?,
        };
        let group_keys = match &group {
            Some(group) => Some(self.group_verifying_keys(group).await?),
            None => None,
        };
        let (pgp, keys, share) = (*self.pgp(), Arc::clone(&self.keys), share.clone());

        let decrypted = rikka_proton_api::offload(move || {
            responses
                .iter()
                .map(|response| match &group_keys {
                    Some(group_keys) => {
                        ShareKey::decrypt_from_group(&pgp, &keys, &share, group_keys, response)
                    }
                    None => ShareKey::decrypt(&pgp, &keys, &share, response),
                })
                .collect::<Result<Vec<ShareKey>>>()
        })
        .await?;

        Ok(ShareKeys::new(decrypted))
    }

    async fn share_group(&self, share: &ShareId) -> Result<Option<GroupId>> {
        if let Some(group) = self.keyring.cached_group(share) {
            return Ok(group);
        }
        let cached = self
            .stored_shares()
            .and_then(|shares| shares.into_iter().find(|cached| cached.share_id() == share));
        match cached {
            Some(cached) => {
                self.keyring.set_share_group(share, cached.group());
                Ok(cached.group())
            }
            None => Ok(self.share_response(share).await?.group()),
        }
    }

    async fn share_key_responses(&self, share: &ShareId) -> Result<Vec<ShareKeyResponse>> {
        let responses: Vec<ShareKeyResponse> = self
            .api()
            .get(format!("{PREFIX}/share/{share}/key"))
            .pages(Paging::numbered(PAGE_SIZE))
            .field_at(&["ShareKeys", "Keys"])
            .collect(Page::<ShareKeyResponse>::new)
            .await?;
        self.cache_share_keys(share, &responses);
        Ok(responses)
    }
}
