use rikka_proton_account::AddressId;
use rikka_proton_api::offload;
use serde::{Deserialize, Serialize};

use crate::anchor::{Anchor, anchored};
use crate::api_errors;
use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::identifier::{LinkId, ShareId, VolumeId};
use crate::node::Node;
use crate::role::ShareTarget;
use crate::share_keys::ShareKeys;

const SHARE_NAME: &str = "New Share";

#[derive(Debug, Clone, Deserialize)]
pub struct SharedByMe {
    #[serde(rename = "ShareID")]
    share_id: ShareId,
    #[serde(rename = "LinkID")]
    link_id: LinkId,
    #[serde(rename = "ContextShareID", default)]
    context_share_id: Option<ShareId>,
}

impl SharedByMe {
    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn link_id(&self) -> &LinkId {
        &self.link_id
    }

    #[must_use]
    pub fn context_share_id(&self) -> Option<&ShareId> {
        self.context_share_id.as_ref()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SharedWithMe {
    #[serde(rename = "VolumeID")]
    volume_id: VolumeId,
    #[serde(rename = "ShareID")]
    share_id: ShareId,
    #[serde(rename = "LinkID")]
    link_id: LinkId,
    #[serde(rename = "ShareTargetType", default)]
    target: ShareTarget,
}

impl SharedWithMe {
    #[must_use]
    pub fn volume_id(&self) -> &VolumeId {
        &self.volume_id
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn link_id(&self) -> &LinkId {
        &self.link_id
    }

    #[must_use]
    pub fn target(&self) -> ShareTarget {
        self.target
    }
}

impl DriveClient {
    pub async fn share_node(&self, context: &ShareKeys, node: &Node) -> Result<ShareKeys> {
        let address_id = context.address_id().clone();
        let client = self.clone();
        let keys = node.keys().clone();
        let created = offload(move || {
            let address_key = crypto::signer(client.keys(), &address_id)?;
            let share = crypto::generate_key_for(
                client.pgp(),
                &[keys.key().public_key(), address_key.public_key()],
                &address_key,
            )?;
            let passphrase_packet = crypto::key_packet(
                client.pgp(),
                keys.passphrase_session_key(),
                &share.key.public_key(),
            )?;
            let name_packet = crypto::key_packet(
                client.pgp(),
                keys.name_session_key(),
                &share.key.public_key(),
            )?;
            Ok::<_, Error>((share, passphrase_packet, name_packet))
        })
        .await?;
        let (share, passphrase_packet, name_packet) = created;

        let request = CreateShareRequest {
            address_id: context.address_id(),
            root_link_id: node.id(),
            share_key: &share.armored_key,
            share_passphrase: &share.armored_passphrase,
            share_passphrase_signature: &share.armored_signature,
            passphrase_key_packet: rikka_proton_api::base64::encode(passphrase_packet),
            name_key_packet: rikka_proton_api::base64::encode(name_packet),
            name: SHARE_NAME,
        };

        let created = self
            .api()
            .post(format!("drive/volumes/{}/shares", context.volume_id()))
            .json(&request)
            .send_field_at::<ShareId>(&["Share", "ID"])
            .await;
        let share_id = match created {
            Ok(created) => created,
            Err(error) => match api_errors::share_conflict(&error) {
                Some(existing_share) => existing_share,
                None => return Err(error.into()),
            },
        };

        self.open_share(&share_id).await
    }

    pub async fn unshare(&self, share: &ShareId, force: bool) -> Result<()> {
        self.api()
            .delete(format!("drive/shares/{share}"))
            .query(&[("Force", rikka_proton_api::flag::to_query_value(force))])
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_editors_can_share(&self, share: &ShareId, allowed: bool) -> Result<()> {
        self.api()
            .put(format!("drive/shares/{share}/editors-can-share"))
            .json(&EditorsCanShareRequest { value: allowed })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn shared_by_me(&self, volume: &VolumeId) -> Result<Vec<SharedByMe>> {
        self.shared_links(format!("drive/v2/volumes/{volume}/shares"))
            .await
    }

    pub async fn shared_with_me(&self) -> Result<Vec<SharedWithMe>> {
        self.shared_links("drive/v2/sharedwithme".to_owned()).await
    }

    async fn shared_links<T: serde::de::DeserializeOwned>(&self, path: String) -> Result<Vec<T>> {
        Ok(self
            .api()
            .get(path)
            .pages(anchored())
            .collect(|response: LinksResponse<T>| response.anchor.page(response.links))
            .await?)
    }
}

#[derive(Debug, Serialize)]
struct CreateShareRequest<'a> {
    #[serde(rename = "AddressID")]
    address_id: &'a AddressId,
    #[serde(rename = "RootLinkID")]
    root_link_id: &'a LinkId,
    #[serde(rename = "ShareKey")]
    share_key: &'a str,
    #[serde(rename = "SharePassphrase")]
    share_passphrase: &'a str,
    #[serde(rename = "SharePassphraseSignature")]
    share_passphrase_signature: &'a str,
    #[serde(rename = "PassphraseKeyPacket")]
    passphrase_key_packet: String,
    #[serde(rename = "NameKeyPacket")]
    name_key_packet: String,
    #[serde(rename = "Name")]
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct EditorsCanShareRequest {
    #[serde(rename = "Value")]
    value: bool,
}

#[derive(Debug, Deserialize)]
#[serde(bound = "T: Deserialize<'de>")]
struct LinksResponse<T> {
    #[serde(rename = "Links", default = "Vec::new")]
    links: Vec<T>,
    #[serde(flatten)]
    anchor: Anchor,
}
