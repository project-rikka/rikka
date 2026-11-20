use jiff::Timestamp;
use rikka_proton_api::{Outcome, Page, Paging, offload};
use serde::{Deserialize, Serialize};

use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::identifier::{LinkId, ShareId, ShareUrlId, VolumeId};
use crate::role::Role;
use crate::share_keys::ShareKeys;
use crate::sharing::share_url::{ShareUrl, ShareUrlContext, ShareUrlSettings, ShareUrlWire};
use crate::sharing::url_password::{self, Sealing, UrlCrypto};

const CONTEXTS_PER_PAGE: u32 = 50;

impl DriveClient {
    pub async fn share_urls(&self, share: &ShareId) -> Result<Vec<ShareUrl>> {
        let urls: Vec<ShareUrlWire> = self
            .api()
            .get(format!("drive/shares/{share}/urls"))
            .send_field_or_default("ShareURLs")
            .await?;
        let client = self.clone();

        offload(move || {
            urls.into_iter()
                .map(|wire| client.decrypt_url(wire))
                .collect()
        })
        .await
    }

    pub async fn create_share_url(
        &self,
        share: &ShareKeys,
        settings: ShareUrlSettings,
    ) -> Result<ShareUrl> {
        let generated = url_password::generate_password();
        let creator = crypto::email(self.keys(), share.address_id())?;
        let sealed = self.sealed_url(share, &generated, &settings).await?;

        let urls_response: ShareUrlWire = self
            .api()
            .post(format!("drive/shares/{}/urls", share.id()))
            .json(&CreateUrlRequest {
                creator_email: &creator,
                sealed: &sealed,
            })
            .send_field("ShareURL")
            .await?;

        Ok(ShareUrl {
            wire: urls_response,
            password: generated,
            custom_password: settings.custom_password,
        })
    }

    pub async fn update_share_url(
        &self,
        share: &ShareKeys,
        url: &ShareUrl,
        settings: ShareUrlSettings,
    ) -> Result<ShareUrl> {
        let generated = url.password().clone();
        let sealed = self.sealed_url(share, &generated, &settings).await?;

        let urls_response: ShareUrlWire = self
            .api()
            .put(format!("drive/shares/{}/urls/{}", share.id(), url.id()))
            .json(&sealed)
            .send_field("ShareURL")
            .await?;

        Ok(ShareUrl {
            wire: urls_response,
            password: generated,
            custom_password: settings.custom_password,
        })
    }

    pub async fn delete_share_url(&self, share: &ShareId, url: &ShareUrlId) -> Result<()> {
        self.api()
            .delete(format!("drive/shares/{share}/urls/{url}"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn delete_share_urls(&self, share: &ShareId, urls: &[ShareUrlId]) -> Result<()> {
        let outcomes: Vec<Outcome<String>> = self
            .api()
            .post(format!("drive/shares/{share}/urls/delete_multiple"))
            .json(&DeleteUrlsRequest { ids: urls })
            .send_field_or_default("Responses")
            .await?;

        match outcomes.into_iter().find(|outcome| !outcome.is_ok()) {
            Some(refused) => Err(Error::Refused {
                code: refused.code(),
                message: refused.error().unwrap_or_default().to_owned(),
            }),
            None => Ok(()),
        }
    }

    pub async fn volume_share_urls(&self, volume: &VolumeId) -> Result<Vec<ShareUrlContext>> {
        let contexts = self
            .api()
            .get(format!("drive/volumes/{volume}/urls"))
            .pages(Paging::numbered(CONTEXTS_PER_PAGE))
            .field("ShareURLContexts")
            .collect(Page::<UrlContextWire>::new)
            .await?;
        let client = self.clone();

        offload(move || {
            contexts
                .into_iter()
                .map(|context| {
                    Ok(ShareUrlContext {
                        context_share_id: context.context_share_id,
                        urls: context
                            .urls
                            .into_iter()
                            .map(|wire| client.decrypt_url(wire))
                            .collect::<Result<Vec<_>>>()?,
                        link_ids: context.link_ids,
                    })
                })
                .collect()
        })
        .await
    }

    async fn sealed_url(
        &self,
        share: &ShareKeys,
        generated: &rikka_proton_pgp::Secret,
        settings: &ShareUrlSettings,
    ) -> Result<UrlCrypto> {
        if settings
            .expires_at
            .is_some_and(|time| time <= Timestamp::now())
        {
            return Err(Error::ExpirationInPast);
        }
        let modulus = self.account().modulus().await?.verify()?;
        let address = crypto::signer(self.keys(), share.address_id())?.public_key();
        let pgp = *self.pgp();
        let session_key = share.session_key().clone();
        let generated = generated.clone();
        let custom = settings.custom_password.clone();
        let role = match settings.editable {
            true => Role::Editor,
            false => Role::Viewer,
        };
        let expiration_time = settings.expires_at;

        offload(move || {
            url_password::sealed(Sealing {
                pgp,
                modulus,
                address,
                session_key: &session_key,
                generated: &generated,
                custom: custom.as_ref(),
                role,
                expiration_time,
            })
        })
        .await
    }

    fn decrypt_url(&self, wire: ShareUrlWire) -> Result<ShareUrl> {
        let address = self
            .keys()
            .address_by_email(&wire.creator)
            .ok_or_else(|| Error::NotOwnAddress(wire.creator.clone()))?;
        let address_keys = self.keys().address_ring(address.id())?;
        let password = crypto::decrypt_text(
            self.pgp(),
            address_keys.private_keys(),
            &wire.password,
            "link password",
        )?;
        let (password, custom_password) = url_password::split(&password, wire.flags);

        Ok(ShareUrl {
            wire,
            password,
            custom_password,
        })
    }
}

#[derive(Debug, Serialize)]
struct CreateUrlRequest<'a> {
    #[serde(rename = "CreatorEmail")]
    creator_email: &'a str,
    #[serde(flatten)]
    sealed: &'a UrlCrypto,
}

#[derive(Debug, Serialize)]
struct DeleteUrlsRequest<'a> {
    #[serde(rename = "ShareURLIDs")]
    ids: &'a [ShareUrlId],
}

#[derive(Debug, Deserialize)]
struct UrlContextWire {
    #[serde(rename = "ContextShareID")]
    context_share_id: ShareId,
    #[serde(rename = "ShareURLs", default)]
    urls: Vec<ShareUrlWire>,
    #[serde(rename = "LinkIDs", default)]
    link_ids: Vec<LinkId>,
}
