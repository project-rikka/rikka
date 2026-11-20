use crate::account_keys::AccountKeys;
use crate::session_info::SessionInfo;

#[derive(Debug)]
pub struct Unlocked {
    session: SessionInfo,
    keys: AccountKeys,
}

impl Unlocked {
    #[must_use]
    pub fn session(&self) -> &SessionInfo {
        &self.session
    }

    #[must_use]
    pub fn keys(&self) -> &AccountKeys {
        &self.keys
    }

    #[must_use]
    pub fn into_keys(self) -> AccountKeys {
        self.keys
    }

    #[must_use]
    pub fn into_parts(self) -> (SessionInfo, AccountKeys) {
        (self.session, self.keys)
    }

    pub(crate) fn new(session: SessionInfo, keys: AccountKeys) -> Unlocked {
        Unlocked { session, keys }
    }
}
