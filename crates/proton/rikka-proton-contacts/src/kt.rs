use rikka_proton_key_transparency::{Pending, RecipientVerdicts, Verdict};

pub const KEY_VERIFICATION_ERROR: &str = "Unable to verify this address at this time";

#[derive(Debug, Clone, Copy)]
pub struct KtVerdicts<'a> {
    pub address: &'a Verdict,
    pub catch_all: Option<&'a Verdict>,
}

impl<'a> From<&'a RecipientVerdicts> for KtVerdicts<'a> {
    fn from(verdicts: &'a RecipientVerdicts) -> KtVerdicts<'a> {
        KtVerdicts {
            address: verdicts.address(),
            catch_all: verdicts.catch_all(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum KtActivation {
    Disabled,
    LogOnly,
    #[default]
    ShowUi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KtAvailability {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KtStatus {
    Verified,
    Unverified,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KtVerification {
    pub status: KtStatus,
    pub keys_changed_recently: bool,
}

impl From<&Verdict> for KtVerification {
    fn from(verdict: &Verdict) -> KtVerification {
        let (status, keys_changed_recently) = match verdict {
            Verdict::Verified => (KtStatus::Verified, false),
            Verdict::Pending(Pending::Keys) => (KtStatus::Verified, true),
            Verdict::Pending(Pending::Obsolescence) => (KtStatus::Unverified, true),
            Verdict::Unverifiable(_) => (KtStatus::Unverified, false),
            Verdict::Failed(_) => (KtStatus::Failed, false),
        };
        KtVerification {
            status,
            keys_changed_recently,
        }
    }
}

impl KtVerification {
    #[must_use]
    pub fn verified() -> KtVerification {
        KtVerification {
            status: KtStatus::Verified,
            keys_changed_recently: false,
        }
    }

    #[must_use]
    pub fn combine(address: Option<&Verdict>, catch_all: Option<&Verdict>) -> KtVerification {
        let failed = [address, catch_all]
            .into_iter()
            .flatten()
            .any(Verdict::is_failed);
        let changed = [address, catch_all]
            .into_iter()
            .flatten()
            .any(Verdict::keys_changed_recently);
        KtVerification {
            status: if failed {
                KtStatus::Failed
            } else {
                KtStatus::Unverified
            },
            keys_changed_recently: changed,
        }
    }

    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.status == KtStatus::Verified
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.status == KtStatus::Failed
    }
}
