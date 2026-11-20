use serde::{Deserialize, Serialize};

use crate::failure::Failure;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReport {
    audited_at: u64,
    next_audit_at: u64,
    #[serde(default)]
    addresses: Vec<AddressAudit>,
    #[serde(default)]
    own_pending: Vec<PendingAudit>,
    #[serde(default)]
    others_pending: Vec<PendingAudit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<AuditError>,
}

impl AuditReport {
    pub(crate) fn completed(
        audited_at: u64,
        next_audit_at: u64,
        addresses: Vec<AddressAudit>,
        own_pending: Vec<PendingAudit>,
        others_pending: Vec<PendingAudit>,
    ) -> AuditReport {
        AuditReport {
            audited_at,
            next_audit_at,
            addresses,
            own_pending,
            others_pending,
            error: None,
        }
    }

    pub(crate) fn interrupted(
        audited_at: u64,
        next_audit_at: u64,
        error: AuditError,
    ) -> AuditReport {
        AuditReport {
            audited_at,
            next_audit_at,
            addresses: Vec::new(),
            own_pending: Vec::new(),
            others_pending: Vec::new(),
            error: Some(error),
        }
    }

    #[must_use]
    pub fn audited_at(&self) -> u64 {
        self.audited_at
    }

    #[must_use]
    pub fn next_audit_at(&self) -> u64 {
        self.next_audit_at
    }

    #[must_use]
    pub fn addresses(&self) -> &[AddressAudit] {
        &self.addresses
    }

    #[must_use]
    pub fn own_pending(&self) -> &[PendingAudit] {
        &self.own_pending
    }

    #[must_use]
    pub fn others_pending(&self) -> &[PendingAudit] {
        &self.others_pending
    }

    #[must_use]
    pub fn error(&self) -> Option<&AuditError> {
        self.error.as_ref()
    }

    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.addresses
            .iter()
            .any(|audit| matches!(audit.outcome, AddressOutcome::Failed(_)))
            || self
                .own_pending
                .iter()
                .chain(&self.others_pending)
                .any(|audit| audit.failure.is_some())
            || self
                .error
                .as_ref()
                .is_some_and(AuditError::retries_exhausted)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressAudit {
    email: String,
    outcome: AddressOutcome,
}

impl AddressAudit {
    pub(crate) fn new(email: impl Into<String>, outcome: AddressOutcome) -> AddressAudit {
        AddressAudit {
            email: email.into(),
            outcome,
        }
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn outcome(&self) -> &AddressOutcome {
        &self.outcome
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressOutcome {
    Passed,
    Warned(AuditWarning),
    Failed(Failure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuditWarning {
    UnverifiableHistory {
        address_was_disabled: bool,
        signature_verification_failed: bool,
    },
    AddressWithoutKeys,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingAudit {
    email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    primary_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    failure: Option<Failure>,
}

impl PendingAudit {
    pub(crate) fn new(
        email: impl Into<String>,
        primary_fingerprint: Option<String>,
        failure: Option<Failure>,
    ) -> PendingAudit {
        PendingAudit {
            email: email.into(),
            primary_fingerprint,
            failure,
        }
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn primary_fingerprint(&self) -> Option<&str> {
        self.primary_fingerprint.as_deref()
    }

    #[must_use]
    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }

    #[must_use]
    pub fn is_passed(&self) -> bool {
        self.failure.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditError {
    #[serde(rename = "failed_trials")]
    failed_attempts: u32,
    #[serde(rename = "too_many_retries")]
    retries_exhausted: bool,
    reason: String,
}

impl AuditError {
    pub(crate) fn new(failed_attempts: u32, retries_exhausted: bool, reason: String) -> AuditError {
        AuditError {
            failed_attempts,
            retries_exhausted,
            reason,
        }
    }

    #[must_use]
    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }

    #[must_use]
    pub fn retries_exhausted(&self) -> bool {
        self.retries_exhausted
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}
