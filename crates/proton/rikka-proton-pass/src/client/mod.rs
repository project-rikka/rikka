mod access;
mod alias;
mod alias_contact;
mod breach;
mod cached;
mod event;
mod extra_password;
mod file;
mod folder;
mod group;
mod import;
mod invite;
mod item;
mod keyring;
mod member;
mod moving;
mod notification;
mod organization;
mod public_keys;
mod secure_link;
mod share;
mod simple_login;
mod vault;

use std::collections::HashMap;
use std::sync::Arc;

use rikka_proton_account::{Account, AccountKeys, AddressId, UserKeys};
use rikka_proton_api::Api;
use rikka_proton_pgp::Pgp;

use crate::api::{FolderResponse, ShareResponse, ShareTarget};
use crate::client::keyring::Keyring;
use crate::crypto::{FolderKey, ShareKeys};
use crate::error::{Error, Result};
use crate::identifier::{FolderId, ShareId};

pub use file::Attachment;

pub(crate) const PREFIX: &str = "pass/v1";
pub(crate) const PAGE_SIZE: u32 = 100;
pub(crate) const SMALL_BATCH: usize = 10;
pub(crate) const LARGE_BATCH: usize = 100;

#[derive(Debug, Clone)]
pub struct PassClient {
    pub(crate) account: Account,
    keys: Arc<UserKeys>,
    account_keys: Option<Arc<AccountKeys>>,
    address: Option<AddressId>,
    addresses: Arc<HashMap<AddressId, UserKeys>>,
    keyring: Arc<Keyring>,
}

impl PassClient {
    #[must_use]
    pub fn new(api: Api, keys: UserKeys) -> PassClient {
        PassClient::with_pgp(api, Pgp::new(), keys)
    }

    #[must_use]
    pub fn with_pgp(api: Api, pgp: Pgp, keys: UserKeys) -> PassClient {
        PassClient::with_account(Account::with_pgp(api, pgp), keys)
    }

    fn with_account(account: Account, keys: UserKeys) -> PassClient {
        PassClient {
            account,
            keys: Arc::new(keys),
            account_keys: None,
            address: None,
            addresses: Arc::default(),
            keyring: Arc::default(),
        }
    }

    pub fn for_account(account: &Account, keys: &Arc<AccountKeys>) -> Result<PassClient> {
        PassClient::with_account(account.clone(), keys.user_signing()?).with_keys(keys)
    }

    pub fn with_keys(mut self, keys: &Arc<AccountKeys>) -> Result<PassClient> {
        self.keys = Arc::new(keys.user_signing()?.with_address_keys(keys));
        self.account_keys = Some(Arc::clone(keys));
        self.addresses = Arc::new(
            keys.addresses()
                .iter()
                .filter_map(|address| {
                    let ring = keys.address_keys(address.id())?;
                    let signer = keys.signing_key(address.id())?;
                    Some((
                        address.id().clone(),
                        UserKeys::new(ring.clone(), signer.clone()),
                    ))
                })
                .collect(),
        );
        if let Ok(address) = keys.primary_address() {
            self.address = Some(address.id().clone());
        }
        Ok(self)
    }

    #[must_use]
    pub fn for_address(mut self, address: AddressId) -> PassClient {
        self.address = Some(address);
        self
    }

    #[must_use]
    pub fn with_address_keys(mut self, address: AddressId, keys: UserKeys) -> PassClient {
        Arc::make_mut(&mut self.addresses).insert(address, keys);
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
    pub fn keys(&self) -> &UserKeys {
        &self.keys
    }

    #[must_use]
    pub fn address(&self) -> Option<&AddressId> {
        self.address.as_ref()
    }

    pub(crate) fn address_keys(&self, address: &AddressId) -> Result<&UserKeys> {
        self.addresses
            .get(address)
            .ok_or_else(|| Error::NoAddressKey {
                address: address.clone(),
            })
    }

    pub(crate) fn account(&self) -> &Account {
        &self.account
    }

    pub(crate) fn account_keys(&self) -> Option<&AccountKeys> {
        self.account_keys.as_deref()
    }

    pub(crate) fn ensure_vault(&self, share: &ShareResponse) -> Result<()> {
        match share.target() {
            ShareTarget::Vault => Ok(()),
            target => Err(Error::NotAVault {
                share: share.share_id().clone(),
                target: target.as_str(),
            }),
        }
    }
}

pub(crate) fn decrypt_folder_keys(
    share: &ShareId,
    folders: &[FolderResponse],
    keys: &ShareKeys,
) -> Result<HashMap<FolderId, FolderKey>> {
    let mut resolver = FolderKeyResolver {
        share,
        folders_by_id: folders
            .iter()
            .map(|folder| (folder.folder_id(), folder))
            .collect(),
        share_keys: keys,
        decrypted: HashMap::new(),
        visiting: Vec::new(),
    };
    for folder in folders {
        resolver.resolve(folder)?;
    }

    Ok(resolver.decrypted)
}

struct FolderKeyResolver<'a> {
    share: &'a ShareId,
    folders_by_id: HashMap<&'a FolderId, &'a FolderResponse>,
    share_keys: &'a ShareKeys,
    decrypted: HashMap<FolderId, FolderKey>,
    visiting: Vec<FolderId>,
}

impl<'a> FolderKeyResolver<'a> {
    fn resolve(&mut self, folder: &'a FolderResponse) -> Result<FolderKey> {
        if let Some(key) = self.decrypted.get(folder.folder_id()) {
            return Ok(key.clone());
        }
        if self.visiting.contains(folder.folder_id()) {
            return Err(Error::FolderCycle {
                share: self.share.clone(),
                folder: folder.folder_id().clone(),
            });
        }

        let parent_key = match folder.parent_folder_id() {
            Some(parent) => {
                let parent_folder =
                    *self
                        .folders_by_id
                        .get(parent)
                        .ok_or_else(|| Error::MissingFolder {
                            share: self.share.clone(),
                            folder: parent.clone(),
                        })?;

                self.visiting.push(folder.folder_id().clone());
                let key = self.resolve(parent_folder)?;
                self.visiting.pop();
                key.key().clone()
            }
            None => self
                .share_keys
                .get(folder.key_rotation())
                .ok_or_else(|| Error::MissingShareKey {
                    share: self.share.clone(),
                    rotation: folder.key_rotation(),
                })?
                .key()
                .clone(),
        };

        let key = crate::crypto::folder::decrypt_key(&parent_key, folder)?;
        self.decrypted
            .insert(folder.folder_id().clone(), key.clone());

        Ok(key)
    }
}
