use std::fmt;

use zeroize::Zeroizing;

#[derive(Clone)]
pub struct MailboxPassphrase(Zeroizing<Vec<u8>>);

impl MailboxPassphrase {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        MailboxPassphrase(Zeroizing::new(bytes))
    }
}

impl AsRef<[u8]> for MailboxPassphrase {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl fmt::Debug for MailboxPassphrase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MailboxPassphrase")
            .finish_non_exhaustive()
    }
}
