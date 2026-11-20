use crate::account::Account;
use crate::error::Result;
use crate::session_info::SessionInfo;
use crate::unlocked::Unlocked;

#[derive(Debug)]
pub struct MailboxPassword {
    account: Account,
    session: SessionInfo,
}

impl MailboxPassword {
    #[must_use]
    pub fn session(&self) -> &SessionInfo {
        &self.session
    }

    pub async fn submit(self, password: impl AsRef<str>) -> Result<Unlocked> {
        let keys = self.account.unlock(password).await?;

        Ok(Unlocked::new(self.session, keys))
    }

    pub(crate) fn new(account: Account, session: SessionInfo) -> MailboxPassword {
        MailboxPassword { account, session }
    }
}
