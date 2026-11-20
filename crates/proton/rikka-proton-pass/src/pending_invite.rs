use std::fmt;

use rikka_proton_account::AddressId;

use crate::api::{ShareTarget, UserInviteResponse};
use crate::content::Vault;
use crate::identifier::{InviteToken, ItemId};

#[derive(Clone)]
pub struct PendingInvite {
    response: UserInviteResponse,
    vault: Option<Vault>,
}

impl PendingInvite {
    pub(crate) fn new(response: UserInviteResponse, vault: Option<Vault>) -> PendingInvite {
        PendingInvite { response, vault }
    }

    #[must_use]
    pub fn token(&self) -> &InviteToken {
        self.response.token()
    }

    #[must_use]
    pub fn inviter_email(&self) -> &str {
        self.response.inviter_email()
    }

    #[must_use]
    pub fn invited_email(&self) -> &str {
        self.response.invited_email()
    }

    #[must_use]
    pub fn invited_address_id(&self) -> &AddressId {
        self.response.invited_address_id()
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
    pub fn reminders_sent(&self) -> i64 {
        self.response.reminders_sent()
    }

    #[must_use]
    pub fn is_from_new_user(&self) -> bool {
        self.response.is_from_new_user()
    }

    #[must_use]
    pub fn create_time(&self) -> jiff::Timestamp {
        self.response.create_time()
    }

    #[must_use]
    pub fn response(&self) -> &UserInviteResponse {
        &self.response
    }
}

impl fmt::Debug for PendingInvite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingInvite")
            .field("token", self.token())
            .field("target", &self.target())
            .field("decrypted", &self.vault.is_some())
            .finish_non_exhaustive()
    }
}
