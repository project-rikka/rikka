mod address;
mod history;
mod pending;
mod report;

use futures_util::future::try_join_all;
use rikka_proton_account::AccountKeys;
use rikka_proton_pgp::PrivateKey;

use crate::client::{EpochCache, KeyTransparency};
use crate::error::{Error, Result};
use crate::timing::{SELF_AUDIT_INTERVAL, SELF_AUDIT_MAX_ATTEMPTS, retry_delay};

pub use report::{
    AddressAudit, AddressOutcome, AuditError, AuditReport, AuditWarning, PendingAudit,
};

impl KeyTransparency {
    pub async fn audit(&self, keys: &AccountKeys) -> Result<AuditReport> {
        self.ensure_available().await?;
        let user_key = keys.primary_user_key().ok_or(Error::NoUserKey)?.clone();
        let started_at = self.now();

        let report = match self.audit_once(keys, &user_key).await {
            Ok(report) => report,
            Err(error) => self.interrupted_report(started_at, &error),
        };
        self.update_state(|state| state.set_last_audit(report.clone()));
        Ok(report)
    }

    async fn audit_once(&self, keys: &AccountKeys, user_key: &PrivateKey) -> Result<AuditReport> {
        let epoch = self.latest(EpochCache::Refresh).await?;

        let resolved = self.check_pending(&epoch).await?;
        let (own_pending, others_pending) = resolved.into_iter().partition(|(entry, _)| {
            keys.addresses()
                .iter()
                .any(|address| address.email() == entry.email())
        });

        let addresses = try_join_all(
            keys.addresses()
                .iter()
                .map(|address| self.audit_address(keys, user_key, &epoch, address.id())),
        )
        .await?
        .into_iter()
        .flatten()
        .collect();

        let now = self.now();
        Ok(AuditReport::completed(
            now,
            now.saturating_add(SELF_AUDIT_INTERVAL),
            addresses,
            pending::pending_audits(own_pending),
            pending::pending_audits(others_pending),
        ))
    }

    fn interrupted_report(&self, started_at: u64, error: &Error) -> AuditReport {
        let failed_attempts = self
            .state()
            .last_audit()
            .and_then(AuditReport::error)
            .map_or(0, AuditError::failed_attempts)
            .saturating_add(1);
        let retries_exhausted = failed_attempts >= SELF_AUDIT_MAX_ATTEMPTS;
        if retries_exhausted || !error.is_unprocessable() {
            tracing::warn!(%error, failed_attempts, "key transparency self-audit failed");
        }
        let delay = if retries_exhausted {
            SELF_AUDIT_INTERVAL
        } else {
            retry_delay(failed_attempts)
        };
        AuditReport::interrupted(
            started_at,
            started_at.saturating_add(delay),
            AuditError::new(
                if retries_exhausted {
                    0
                } else {
                    failed_attempts
                },
                retries_exhausted,
                error.to_string(),
            ),
        )
    }
}
