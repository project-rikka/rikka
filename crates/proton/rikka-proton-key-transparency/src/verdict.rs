use crate::failure::Failure;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Verified,
    Pending(Pending),
    Unverifiable(Unverifiable),
    Failed(Failure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pending {
    Keys,
    Obsolescence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unverifiable {
    Absent,
    Obsolete,
    ExcludedDomain,
    NoSignedKeyList,
    NeverIncluded,
}

impl Verdict {
    #[must_use]
    pub fn is_verified(&self) -> bool {
        matches!(self, Verdict::Verified)
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self, Verdict::Failed(_))
    }

    #[must_use]
    pub fn keys_changed_recently(&self) -> bool {
        matches!(self, Verdict::Pending(_))
    }

    #[must_use]
    pub fn failure(&self) -> Option<&Failure> {
        match self {
            Verdict::Failed(failure) => Some(failure),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipientVerdicts {
    address: Verdict,
    catch_all: Option<Verdict>,
}

impl RecipientVerdicts {
    pub(crate) fn new(address: Verdict, catch_all: Option<Verdict>) -> RecipientVerdicts {
        RecipientVerdicts { address, catch_all }
    }

    #[must_use]
    pub fn address(&self) -> &Verdict {
        &self.address
    }

    #[must_use]
    pub fn catch_all(&self) -> Option<&Verdict> {
        self.catch_all.as_ref()
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.address.is_failed() || self.catch_all.as_ref().is_some_and(Verdict::is_failed)
    }

    #[must_use]
    pub fn keys_changed_recently(&self) -> bool {
        self.address.keys_changed_recently()
            || self
                .catch_all
                .as_ref()
                .is_some_and(Verdict::keys_changed_recently)
    }
}
