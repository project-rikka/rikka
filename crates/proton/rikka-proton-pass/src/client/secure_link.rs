use std::collections::HashMap;

use crate::api::CreatedSecureLink;
use crate::api::{CreateSecureLinkRequest, SecureLinkResponse};
use crate::client::{PREFIX, PassClient};
use crate::crypto::{EncryptionKey, ItemKey};
use crate::error::{Error, Result};
use crate::identifier::{ItemId, SecureLinkId, ShareId};
use crate::new_secure_link::NewSecureLink;
use crate::pass_item::PassItem;
use crate::secure_link::SecureLink;

impl PassClient {
    pub async fn create_secure_link(
        &self,
        item: &PassItem,
        link: NewSecureLink,
    ) -> Result<SecureLink> {
        let item_key = item.key().clone();
        let link_keys = rikka_proton_api::offload({
            let key = item_key.key().clone();
            move || crate::crypto::secure_link::generate_link_keys(&key)
        })
        .await?;

        let request = CreateSecureLinkRequest {
            revision: item.revision(),
            expiration_time: link.expires_in_seconds(),
            max_read_count: link.read_limit(),
            encrypted_item_key: link_keys.encrypted_item_key,
            encrypted_link_key: link_keys.encrypted_link_key,
            link_key_share_key_rotation: item_key.rotation(),
            link_key_encrypted_with_item_key: true,
        };

        let (share, id) = (item.share_id(), item.id());
        let envelope: CreatedSecureLink = self
            .api()
            .post(format!("{PREFIX}/share/{share}/item/{id}/public_link"))
            .json(&request)
            .send_field("PublicLink")
            .await?;
        let created = envelope;

        Ok(SecureLink {
            id: created.id,
            share: share.clone(),
            item: id.clone(),
            address: crate::crypto::secure_link::link_url(&created.url, &link_keys.link_key),
            active: true,
            expiration_time: created.expiration_time,
            read_count: 0,
            max_read_count: link.read_limit().map(i64::from),
        })
    }

    pub async fn secure_links(&self) -> Result<Vec<SecureLink>> {
        let envelope: Vec<SecureLinkResponse> = self
            .api()
            .get(format!("{PREFIX}/public_link"))
            .send_field_or_default("PublicLinks")
            .await?;

        let mut item_keys: HashMap<(ShareId, ItemId), Vec<ItemKey>> = HashMap::new();
        let mut links = Vec::with_capacity(envelope.len());

        for response in envelope {
            let rotation = response.link_key_share_key_rotation;
            let parent = if response
                .link_key_encrypted_with_item_key
                .unwrap_or_default()
            {
                let link_item = (response.share_id.clone(), response.item_id.clone());
                if !item_keys.contains_key(&link_item) {
                    let keys = self
                        .item_keys(&response.share_id, &response.item_id)
                        .await?;
                    item_keys.insert(link_item.clone(), keys);
                }
                item_keys[&link_item]
                    .iter()
                    .find(|key| key.rotation() == rotation)
                    .map(|key| key.key().clone())
                    .ok_or_else(|| Error::MissingItemKey {
                        item: response.item_id.clone(),
                        rotation,
                    })?
            } else {
                self.share_key(&response.share_id, rotation)
                    .await?
                    .ok_or_else(|| Error::MissingShareKey {
                        share: response.share_id.clone(),
                        rotation,
                    })?
                    .key()
                    .clone()
            };

            links.push(decrypt_link(&parent, response)?);
        }

        Ok(links)
    }

    pub async fn delete_secure_link(&self, link: &SecureLinkId) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/public_link/{link}"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_inactive_secure_links(&self) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/public_link/inactive"))
            .send_unit()
            .await?;

        Ok(())
    }
}

fn decrypt_link(parent: &EncryptionKey, response: SecureLinkResponse) -> Result<SecureLink> {
    let link_key =
        crate::crypto::secure_link::decrypt_link_key(parent, &response.encrypted_link_key)?;

    Ok(SecureLink {
        address: crate::crypto::secure_link::link_url(&response.url, &link_key),
        id: response.id,
        share: response.share_id,
        item: response.item_id,
        active: response.active,
        expiration_time: response.expiration_time,
        read_count: response.read_count,
        max_read_count: response.max_read_count,
    })
}
