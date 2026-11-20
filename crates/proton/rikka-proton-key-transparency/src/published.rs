use rikka_proton_account::PublicKeys;

use crate::verdict::RecipientVerdicts;

#[derive(Debug, Clone)]
pub struct VerifiedPublicKeys {
    keys: PublicKeys,
    verdicts: RecipientVerdicts,
}

impl VerifiedPublicKeys {
    pub(crate) fn new(keys: PublicKeys, verdicts: RecipientVerdicts) -> VerifiedPublicKeys {
        VerifiedPublicKeys { keys, verdicts }
    }

    #[must_use]
    pub fn keys(&self) -> &PublicKeys {
        &self.keys
    }

    #[must_use]
    pub fn verdicts(&self) -> &RecipientVerdicts {
        &self.verdicts
    }

    #[must_use]
    pub fn into_parts(self) -> (PublicKeys, RecipientVerdicts) {
        (self.keys, self.verdicts)
    }
}
