use rikka_proton_pgp::SignatureStatus;

#[derive(Debug, Clone)]
pub struct Signatures {
    passphrase: SignatureStatus,
    name: SignatureStatus,
    content: SignatureStatus,
    attributes: SignatureStatus,
}

impl Signatures {
    #[must_use]
    pub fn passphrase(&self) -> &SignatureStatus {
        &self.passphrase
    }

    #[must_use]
    pub fn name(&self) -> &SignatureStatus {
        &self.name
    }

    #[must_use]
    pub fn content(&self) -> &SignatureStatus {
        &self.content
    }

    #[must_use]
    pub fn attributes(&self) -> &SignatureStatus {
        &self.attributes
    }

    #[must_use]
    pub fn all_valid(&self) -> bool {
        [
            &self.passphrase,
            &self.name,
            &self.content,
            &self.attributes,
        ]
        .iter()
        .all(|status| status.is_valid() || status.is_missing())
    }

    pub(crate) fn new(
        passphrase: SignatureStatus,
        name: SignatureStatus,
        content: SignatureStatus,
        attributes: SignatureStatus,
    ) -> Signatures {
        Signatures {
            passphrase,
            name,
            content,
            attributes,
        }
    }
}
