use std::fmt;

use crate::api::{GroupInviteResponse, ShareTarget};
use crate::content::Vault;
use crate::identifier::{GroupId, InviteToken, ItemId};

#[derive(Clone)]
pub struct PendingGroupInvite {
    response: GroupInviteResponse,
    vault: Option<Vault>,
}

impl PendingGroupInvite {
    pub(crate) fn new(response: GroupInviteResponse, vault: Option<Vault>) -> PendingGroupInvite {
        PendingGroupInvite { response, vault }
    }

    #[must_use]
    pub fn token(&self) -> &InviteToken {
        self.response.token()
    }

    #[must_use]
    pub fn group_id(&self) -> &GroupId {
        self.response.group_id()
    }

    #[must_use]
    pub fn group_email(&self) -> &str {
        self.response.invited_email()
    }

    #[must_use]
    pub fn inviter_email(&self) -> &str {
        self.response.inviter_email()
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
    pub fn vault(&self) -> Option<&Vault> {
        self.vault.as_ref()
    }

    #[must_use]
    pub fn member_count(&self) -> Option<i64> {
        self.response.vault_data().map(|data| data.member_count())
    }

    #[must_use]
    pub fn item_count(&self) -> Option<i64> {
        self.response.vault_data().map(|data| data.item_count())
    }

    #[must_use]
    pub fn is_group_owner(&self) -> bool {
        self.response.is_group_owner()
    }

    #[must_use]
    pub fn create_time(&self) -> jiff::Timestamp {
        self.response.create_time()
    }

    #[must_use]
    pub fn response(&self) -> &GroupInviteResponse {
        &self.response
    }
}

impl fmt::Debug for PendingGroupInvite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingGroupInvite")
            .field("token", self.token())
            .field("group", self.group_id())
            .field("decrypted", &self.vault.is_some())
            .finish_non_exhaustive()
    }
}
