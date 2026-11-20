use rikka_proton_pgp::Secret;

use crate::account::Account;
use crate::error::{Error, Result};
use crate::mailbox::MailboxPassword;
use crate::session_info::SessionInfo;
use crate::sso::Sso;
use crate::two_factor::TwoFactor;
use crate::unlocked::Unlocked;

#[derive(Debug)]
#[non_exhaustive]
pub enum LoginOutcome {
    Unlocked(Box<Unlocked>),
    TwoFactorRequired(Box<TwoFactor>),
    MailboxPasswordRequired(Box<MailboxPassword>),
    Sso(Box<Sso>),
}

impl LoginOutcome {
    pub fn into_unlocked(self) -> Result<Unlocked> {
        match self {
            LoginOutcome::Unlocked(unlocked) => Ok(*unlocked),
            LoginOutcome::TwoFactorRequired(_) => Err(Error::SecondFactorRequired),
            LoginOutcome::MailboxPasswordRequired(_) => Err(Error::MailboxPasswordRequired),
            LoginOutcome::Sso(_) => Err(Error::IdentityProviderRequired),
        }
    }

    #[must_use]
    pub fn session(&self) -> Option<&SessionInfo> {
        match self {
            LoginOutcome::Unlocked(unlocked) => Some(unlocked.session()),
            LoginOutcome::TwoFactorRequired(two_factor) => Some(two_factor.session()),
            LoginOutcome::MailboxPasswordRequired(mailbox) => Some(mailbox.session()),
            LoginOutcome::Sso(_) => None,
        }
    }
}

pub(crate) async fn complete_login(
    account: Account,
    session: SessionInfo,
    password: Secret,
) -> Result<LoginOutcome> {
    if session.password_mode().needs_mailbox_password() {
        return Ok(LoginOutcome::MailboxPasswordRequired(Box::new(
            MailboxPassword::new(account, session),
        )));
    }

    let keys = account.unlock(password.expose()).await?;

    Ok(LoginOutcome::Unlocked(Box::new(Unlocked::new(
        session, keys,
    ))))
}
