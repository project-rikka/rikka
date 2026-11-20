use crate::audit::report::PendingAudit;
use crate::client::KeyTransparency;
use crate::epoch::Epoch;
use crate::error::{Error, Result};
use crate::failure::Failure;
use crate::identifier::Identifier;
use crate::proof::ProofKind;
use crate::routes;
use crate::state::PendingKeyList;
use crate::timing::{is_past_certificate_lifetime, is_past_epoch_interval};

enum PendingCheck {
    Resolved,
    Deferred,
}

impl KeyTransparency {
    pub(crate) async fn check_pending(
        &self,
        epoch: &Epoch,
    ) -> Result<Vec<(PendingKeyList, Option<Failure>)>> {
        let entries = self.state().pending().to_vec();
        let mut resolved = Vec::new();
        for entry in entries {
            match self.check_entry(epoch, &entry).await {
                Ok(PendingCheck::Deferred) => {}
                Ok(PendingCheck::Resolved) => resolved.push((entry, None)),
                Err(Error::Failed(failure)) => resolved.push((entry, Some(failure))),
                Err(error) => return Err(error),
            }
        }
        self.update_state(|state| {
            for (entry, _) in &resolved {
                state.remove_pending(entry);
            }
        });
        Ok(resolved)
    }

    async fn check_entry(&self, epoch: &Epoch, entry: &PendingKeyList) -> Result<PendingCheck> {
        let now = self.now();
        let created_at = entry.created_at();
        if !is_past_epoch_interval(created_at, now) && entry.expected_min_epoch_id() > epoch.id() {
            return Ok(PendingCheck::Deferred);
        }

        let identifier = Identifier::for_scope(entry.email(), entry.scope())?;
        let revision = entry.revision();
        let proof = routes::proof(self.api(), epoch.id(), &identifier, revision).await?;

        if proof.kind() == ProofKind::Absence {
            return if is_past_certificate_lifetime(created_at, now) {
                Ok(PendingCheck::Resolved)
            } else if is_past_epoch_interval(created_at, now) {
                Err(Failure::PendingKeyListIgnored { revision }.into())
            } else {
                Ok(PendingCheck::Deferred)
            };
        }

        let included = routes::signed_key_list(self.api(), revision, &identifier)
            .await?
            .ok_or(Failure::PendingKeyListMissing { revision })?;
        let content = Some(entry.content());
        if included.data() != content && included.obsolescence_token() != content {
            return Err(Failure::PendingKeyListChanged { revision }.into());
        }
        proof.verify_inclusion(self.realm(), epoch.tree_hash(), &identifier, &included)?;
        Ok(PendingCheck::Resolved)
    }
}

pub(crate) fn pending_audits(
    resolved: Vec<(PendingKeyList, Option<Failure>)>,
) -> Vec<PendingAudit> {
    resolved
        .into_iter()
        .map(|(entry, failure)| {
            PendingAudit::new(entry.email(), entry.primary_fingerprint(), failure)
        })
        .collect()
}
