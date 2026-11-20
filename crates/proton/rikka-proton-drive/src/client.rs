use std::sync::Arc;

use rikka_proton_account::{Account, AccountKeys, AddressId};
use rikka_proton_api::{Api, offload};
use rikka_proton_cache::Wired;
use rikka_proton_pgp::{Pgp, PublicKey};

use crate::crypto::{self, DecryptedKey};
use crate::error::{Error, Result};
use crate::identifier::{ShareId, VolumeId};
use crate::share::Share;
use crate::share_keys::ShareKeys;
use crate::volume::Volume;

#[derive(Debug, Clone)]
pub struct DriveClient {
    pub(crate) account: Account,
    keys: Arc<AccountKeys>,
    client_uid: Option<Arc<str>>,
}

impl DriveClient {
    #[must_use]
    pub fn new(api: Api, keys: Arc<AccountKeys>) -> DriveClient {
        DriveClient::for_account(&Account::new(api), &keys)
    }

    #[must_use]
    pub fn with_pgp(api: Api, pgp: Pgp, keys: Arc<AccountKeys>) -> DriveClient {
        DriveClient::for_account(&Account::with_pgp(api, pgp), &keys)
    }

    #[must_use]
    pub fn for_account(account: &Account, keys: &Arc<AccountKeys>) -> DriveClient {
        DriveClient {
            account: account.clone(),
            keys: Arc::clone(keys),
            client_uid: None,
        }
    }

    #[must_use]
    pub fn with_keys(mut self, keys: &Arc<AccountKeys>) -> DriveClient {
        self.keys = Arc::clone(keys);
        self
    }

    #[must_use]
    pub fn with_client_uid(mut self, uid: impl Into<String>) -> DriveClient {
        self.client_uid = Some(Arc::from(uid.into()));
        self
    }

    #[must_use]
    pub fn api(&self) -> &Api {
        self.account.api()
    }

    #[must_use]
    pub fn pgp(&self) -> &Pgp {
        self.account.pgp()
    }

    #[must_use]
    pub fn keys(&self) -> &AccountKeys {
        &self.keys
    }

    #[must_use]
    pub fn client_uid(&self) -> Option<&str> {
        self.client_uid.as_deref()
    }

    pub async fn volumes(&self) -> Result<Vec<Volume>> {
        let response: Vec<Wired<Volume>> = self
            .api()
            .get("drive/volumes")
            .send_field_or_default("Volumes")
            .await?;
        self.cache_volumes(&response);

        Ok(response.into_iter().map(Wired::into_value).collect())
    }

    pub async fn volume(&self, id: &VolumeId) -> Result<Volume> {
        Ok(self
            .api()
            .get(format!("drive/volumes/{id}"))
            .send_field("Volume")
            .await?)
    }

    pub async fn shares(&self) -> Result<Vec<Share>> {
        Ok(self
            .api()
            .get("drive/shares")
            .send_field_or_default("Shares")
            .await?)
    }

    pub async fn share(&self, id: &ShareId) -> Result<Share> {
        let response: Wired<Share> = self.api().get(format!("drive/shares/{id}")).send().await?;
        self.cache_share(&response);

        Ok(response.into_value())
    }

    pub async fn open_share(&self, id: &ShareId) -> Result<ShareKeys> {
        let share = self.share(id).await?;
        let creator = share
            .creator()
            .filter(|creator| self.keys.address_by_email(creator).is_none())
            .map(str::to_owned);
        let published = match creator {
            Some(creator) => self.published_keys(&creator).await?.verification,
            None => Vec::new(),
        };
        let client = self.clone();

        offload(move || client.decrypt_share_keys(share, &published)).await
    }

    pub fn decrypt_share(&self, share: Share) -> Result<ShareKeys> {
        self.decrypt_share_keys(share, &[])
    }

    fn decrypt_share_keys(&self, share: Share, published: &[PublicKey]) -> Result<ShareKeys> {
        let armored_key = share.key().ok_or(Error::Incomplete("share key"))?;
        let armored_passphrase = share
            .passphrase()
            .ok_or(Error::Incomplete("share passphrase"))?;

        let membership = share
            .memberships()
            .iter()
            .find(|membership| self.keys.address_keys(membership.address_id()).is_some());
        let address_id = match (membership, share.address_id()) {
            (Some(membership), _) => membership.address_id().clone(),
            (None, Some(address)) => address.clone(),
            (None, None) => return Err(Error::NoShareAddress(share.id().clone())),
        };
        let address_keys = self.keys.address_ring(&address_id)?;

        let mut verification = match crypto::verification_keys(&self.keys, share.creator()) {
            keys if keys.is_empty() => crypto::verification_keys(
                &self.keys,
                self.keys
                    .address(&address_id)
                    .map(|address| address.email()),
            ),
            keys => keys,
        };
        verification.extend(published.iter().cloned());

        let share_key: DecryptedKey = match membership {
            Some(membership) => {
                let packet = rikka_proton_api::base64::decode(membership.key_packet())
                    .map_err(|_| Error::NotBase64("membership key packet"))?;
                let session_key =
                    crypto::decrypt_session_key(self.pgp(), address_keys.private_keys(), &packet)?;
                crypto::decrypt_key_with_session_key(
                    self.pgp(),
                    armored_key,
                    armored_passphrase,
                    session_key,
                    share.passphrase_signature(),
                    &verification,
                )?
            }
            None => crypto::decrypt_key(
                self.pgp(),
                armored_key,
                armored_passphrase,
                share.passphrase_signature(),
                address_keys.private_keys(),
                &verification,
            )?,
        };
        Error::check_signature("share passphrase", share_key.signature.clone())?;

        Ok(ShareKeys::new(
            share.id().clone(),
            share.volume_id().clone(),
            share.root_id().clone(),
            address_id,
            share_key,
        ))
    }

    pub(crate) fn account(&self) -> &Account {
        &self.account
    }

    pub(crate) fn primary_address(&self) -> Result<AddressId> {
        Ok(self.keys.primary_address()?.id().clone())
    }
}
