pub(crate) mod authentication;
pub(crate) mod challenge;
pub(crate) mod hash_version;
pub(crate) mod mailbox_passphrase;
pub(crate) mod modulus;
pub(crate) mod password_verifier;
pub(crate) mod proofs;
pub(crate) mod registration;

use crate::error::Result;
use crate::srp::authentication::Authentication;
use crate::srp::mailbox_passphrase::MailboxPassphrase;
use crate::srp::registration::Registration;

#[derive(Debug, Clone, Copy, Default)]
pub struct Srp;

impl Srp {
    #[must_use]
    pub fn new() -> Self {
        Srp
    }

    pub fn authenticate(&self) -> Authentication {
        Authentication::new()
    }

    pub fn register(&self) -> Registration {
        Registration::new()
    }

    pub fn mailbox_passphrase(
        &self,
        password: &str,
        salt: impl AsRef<[u8]>,
    ) -> Result<MailboxPassphrase> {
        let hash = proton_srp::mailbox_password_hash(password, salt.as_ref())?;
        Ok(MailboxPassphrase::new(hash.hashed_password().to_vec()))
    }
}
