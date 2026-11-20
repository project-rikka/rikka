use rikka_proton_pgp::SecretBytes;

use crate::crypto::key::AuthenticatorKey;
use crate::entry::Entry;
use crate::error::{Error, Result};

const CONTENT_AAD: &[u8] = b"entrycontent";

impl AuthenticatorKey {
    pub fn seal(&self, entry: &Entry) -> Result<String> {
        let plaintext = SecretBytes::new(entry.encode());
        let sealed = self.material().seal(plaintext.expose(), CONTENT_AAD)?;

        Ok(rikka_proton_api::base64::encode(sealed))
    }

    pub fn open(&self, content: &str) -> Result<Entry> {
        let sealed =
            rikka_proton_api::base64::decode(content).map_err(|_| Error::MalformedContent)?;

        let plaintext = SecretBytes::new(
            self.material()
                .open(&sealed, CONTENT_AAD)
                .map_err(|_| Error::UndecryptableContent)?,
        );

        Entry::decode(plaintext.expose())
    }
}
