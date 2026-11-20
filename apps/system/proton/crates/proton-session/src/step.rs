use rikka_proton_account::{MailboxPassword, TwoFactor};
use rikka_proton_client::Session;

use crate::access::Access;
use crate::error::Result;

#[derive(Debug)]
pub enum Step {
    Signed(Box<Session>),
    SecondFactor(Box<SecondFactor>),
    MailboxPassword(Box<MailboxKey>),
}

#[derive(Debug)]
pub struct SecondFactor {
    access: Access,
    pending: TwoFactor,
}

impl SecondFactor {
    pub(crate) fn new(access: Access, pending: TwoFactor) -> SecondFactor {
        SecondFactor { access, pending }
    }

    #[must_use]
    pub fn has_key(&self) -> bool {
        self.pending.has_fido2()
    }

    pub async fn totp(self, code: impl Into<String>) -> Result<Step> {
        let outcome = self.pending.totp(code).await?;
        self.access.advance(outcome).await
    }
}

#[derive(Debug)]
pub struct MailboxKey {
    access: Access,
    pending: MailboxPassword,
}

impl MailboxKey {
    pub(crate) fn new(access: Access, pending: MailboxPassword) -> MailboxKey {
        MailboxKey { access, pending }
    }

    pub async fn submit(self, password: impl AsRef<str>) -> Result<Step> {
        let unlocked = self.pending.submit(password).await?;
        Ok(Step::Signed(Box::new(
            self.access.finish_sign_in(unlocked.into_keys()).await,
        )))
    }
}
