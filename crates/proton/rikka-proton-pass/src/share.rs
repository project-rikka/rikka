use std::fmt;

use rikka_proton_account::AddressId;

use crate::api::{ShareResponse, ShareRole, ShareTarget};
use crate::content::Vault;
use crate::identifier::{FolderId, ItemId, ShareId, VaultId};

#[derive(Clone)]
pub struct Share {
    response: ShareResponse,
    vault: Option<Vault>,
}

impl Share {
    pub(crate) fn new(response: ShareResponse, vault: Option<Vault>) -> Share {
        Share { response, vault }
    }

    #[must_use]
    pub fn id(&self) -> &ShareId {
        self.response.share_id()
    }

    #[must_use]
    pub fn vault_id(&self) -> &VaultId {
        self.response.vault_id()
    }

    #[must_use]
    pub fn address_id(&self) -> &AddressId {
        self.response.address_id()
    }

    #[must_use]
    pub fn target(&self) -> ShareTarget {
        self.response.target()
    }

    #[must_use]
    pub fn item_id(&self) -> Option<ItemId> {
        match self.response.target() {
            ShareTarget::Item => Some(ItemId::from(self.response.target_id())),
            _ => None,
        }
    }

    #[must_use]
    pub fn folder_id(&self) -> Option<FolderId> {
        match self.response.target() {
            ShareTarget::Folder => Some(FolderId::from(self.response.target_id())),
            _ => None,
        }
    }

    #[must_use]
    pub fn vault(&self) -> Option<&Vault> {
        self.vault.as_ref()
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.vault.as_ref().map(|vault| vault.name.as_str())
    }

    #[must_use]
    pub fn role(&self) -> ShareRole {
        self.response.role()
    }

    #[must_use]
    pub fn is_owner(&self) -> bool {
        self.response.is_owner()
    }

    #[must_use]
    pub fn is_shared(&self) -> bool {
        self.response.is_shared()
    }

    #[must_use]
    pub fn key_rotation(&self) -> Option<i64> {
        self.response.content_key_rotation()
    }

    #[must_use]
    pub fn response(&self) -> &ShareResponse {
        &self.response
    }

    #[must_use]
    pub fn into_response(self) -> ShareResponse {
        self.response
    }
}

impl fmt::Debug for Share {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Share")
            .field("id", self.id())
            .field("target", &self.target())
            .field("decrypted", &self.vault.is_some())
            .finish_non_exhaustive()
    }
}
