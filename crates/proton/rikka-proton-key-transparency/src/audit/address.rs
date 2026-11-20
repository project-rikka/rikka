use rikka_proton_account::{
    AccountKeys, ActiveKey, AddressId, AddressKeyFlags, Error as AccountError, SignedKeyList,
    SignedKeyListItem,
};
use rikka_proton_api::offload;
use rikka_proton_pgp::{PrivateKey, PublicKey};

use crate::audit::history::{
    History, Revision, RevisionStatus, history_outcome, is_non_decreasing,
};
use crate::audit::report::{AddressAudit, AddressOutcome, AuditWarning};
use crate::client::KeyTransparency;
use crate::epoch::Epoch;
use crate::error::{Error, Result};
use crate::failure::{self, Failure};
use crate::identifier::Identifier;
use crate::routes::{self, AuditedAddress};
use crate::state::PendingKeyList;
use crate::verified_epoch::VerifiedEpoch;

const KEY_VERSION_4: u8 = 4;
const LATEST_SIGNATURE_TIME: u64 = u32::MAX as u64;

impl KeyTransparency {
    pub(crate) async fn audit_address(
        &self,
        keys: &AccountKeys,
        user_key: &PrivateKey,
        epoch: &Epoch,
        address_id: &AddressId,
    ) -> Result<Option<AddressAudit>> {
        let address = routes::address(self.api(), address_id).await?;
        if address.is_disabled() {
            return Ok(None);
        }
        let outcome = match self.evaluate_address(keys, user_key, epoch, &address).await {
            Ok(outcome) => outcome,
            Err(Error::Failed(failure)) => AddressOutcome::Failed(failure),
            Err(error) => return Err(error),
        };
        Ok(Some(AddressAudit::new(address.email, outcome)))
    }

    async fn evaluate_address(
        &self,
        keys: &AccountKeys,
        user_key: &PrivateKey,
        epoch: &Epoch,
        address: &AuditedAddress,
    ) -> Result<AddressOutcome> {
        let own_keys = own_active_keys(keys, address);
        if address.keys.is_empty() {
            self.audit_address_without_keys(epoch, address).await?;
            return Ok(AddressOutcome::Warned(AuditWarning::AddressWithoutKeys));
        }
        let Some(current) = &address.signed_key_list else {
            self.publish_initial_key_list(epoch, address, &own_keys)
                .await?;
            return Ok(AddressOutcome::Passed);
        };
        if current.revision() == 0 {
            return Err(Failure::MissingRevision.into());
        }

        let identifier = Identifier::address(&address.email)?;
        let verified_epoch = routes::verified_epoch(self.api(), &address.id)
            .await?
            .and_then(|signed| signed.verify(self.pgp(), keys.user_keys()));
        if let Some(verified) = verified_epoch
            && self
                .extend_verified_epoch(&verified, epoch, address, &identifier, current, user_key)
                .await?
        {
            return Ok(AddressOutcome::Passed);
        }

        let verification_keys: Vec<PublicKey> = own_keys
            .iter()
            .filter(|own| own.flags().can_verify())
            .map(|own| own.key().public_key())
            .collect();
        let revisions = self
            .audit_revisions(epoch, &identifier, verified_epoch, &verification_keys)
            .await?;

        let data = current.data().ok_or(Failure::IncompleteSignedKeyList)?;
        let signature_was_in_future = self
            .check_current_list(
                address,
                current,
                data,
                &revisions,
                &own_keys,
                &verification_keys,
            )
            .await?;
        check_listed_keys(data, &own_keys)?;

        if let Some(last) = revisions.last()
            && let RevisionStatus::Verified { signed_at } = last.status
        {
            self.upload_verified_epoch(
                &address.id,
                VerifiedEpoch::new(epoch.id(), last.revision, signed_at),
                user_key,
            )
            .await?;
        }

        Ok(history_outcome(&revisions, signature_was_in_future))
    }

    async fn audit_revisions(
        &self,
        epoch: &Epoch,
        identifier: &Identifier,
        verified: Option<VerifiedEpoch>,
        verification_keys: &[PublicKey],
    ) -> Result<Vec<Revision>> {
        let verified_revision = verified.map_or(0, |verified| verified.revision());
        let history = routes::signed_key_lists(self.api(), verified_revision, identifier).await?;
        let last_included = history
            .iter()
            .filter(|list| list.min_epoch_id().is_some_and(|id| id <= epoch.id()))
            .map(SignedKeyList::revision)
            .max()
            .unwrap_or(verified_revision)
            .max(verified_revision);
        let first = verified_revision.saturating_add(1);
        let next = last_included.saturating_add(1);
        let proofs = self.proofs(epoch, identifier, first..=next).await?;

        let auditor = History {
            realm: self.realm().clone(),
            pgp: *self.pgp(),
            tree_hash: epoch.tree_hash().to_owned(),
            identifier: identifier.clone(),
            verification_keys: verification_keys.to_vec(),
            now: self.now(),
        };
        let revisions = {
            let history = history.clone();
            let proofs = proofs.clone();
            offload(move || {
                (first..=last_included)
                    .zip(&proofs)
                    .map(|(revision, proof)| {
                        let listed = history.iter().find(|list| list.revision() == revision);
                        auditor.audit(revision, listed, proof)
                    })
                    .collect::<Result<Vec<Revision>, Failure>>()
            })
            .await?
        };

        let start_epoch = verified.map_or(0, |verified| verified.epoch_id());
        if !is_non_decreasing(
            history.iter().filter_map(SignedKeyList::min_epoch_id),
            start_epoch,
        ) {
            return Err(Failure::MinEpochsNotIncreasing.into());
        }
        let signed_times = revisions
            .iter()
            .filter_map(|revision| match revision.status {
                RevisionStatus::Verified { signed_at } => Some(signed_at),
                _ => None,
            });
        if !is_non_decreasing(
            signed_times,
            verified.map_or(0, |verified| verified.signed_at()),
        ) {
            return Err(Failure::SignatureTimesNotIncreasing.into());
        }
        let next_proof = proofs.last().ok_or(Failure::IncompleteSignedKeyList)?;
        next_proof.verify_absence(self.realm(), epoch.tree_hash(), identifier, next)?;
        Ok(revisions)
    }

    async fn check_current_list(
        &self,
        address: &AuditedAddress,
        current: &SignedKeyList,
        data: &str,
        revisions: &[Revision],
        own_keys: &[ActiveKey],
        verification_keys: &[PublicKey],
    ) -> Result<bool> {
        if let Some(audited) = revisions
            .iter()
            .find(|revision| revision.revision == current.revision())
        {
            if audited.data.as_deref() != Some(data) {
                return Err(Failure::AuditedKeyListMismatch.into());
            }
            if matches!(audited.status, RevisionStatus::Verified { .. }) {
                return Ok(false);
            }
            self.resign_future_signature(address, current, own_keys)
                .await?;
            return Ok(true);
        }

        if current.signature().is_none() {
            return Err(Failure::IncompleteSignedKeyList.into());
        }
        let in_future = current
            .signed_at(self.pgp(), verification_keys, self.now())
            .is_none();
        if in_future {
            self.resign_future_signature(address, current, own_keys)
                .await?;
        }
        let expected = current
            .min_epoch_id()
            .or(current.expected_min_epoch_id())
            .ok_or(Failure::MissingExpectedMinEpoch)?;
        self.add_pending(
            PendingKeyList::new(
                self.now(),
                &address.email,
                data,
                current.revision(),
                expected,
            )
            .with_address(address.id.clone()),
        );
        Ok(in_future)
    }

    async fn extend_verified_epoch(
        &self,
        verified: &VerifiedEpoch,
        epoch: &Epoch,
        address: &AuditedAddress,
        identifier: &Identifier,
        current: &SignedKeyList,
        user_key: &PrivateKey,
    ) -> Result<bool> {
        if verified.epoch_id() == epoch.id() {
            return Ok(true);
        }
        if verified.revision() != current.revision() {
            return Ok(false);
        }
        let next = current.revision().saturating_add(1);
        let proof = routes::proof(self.api(), epoch.id(), identifier, next).await?;
        proof.verify_absence(self.realm(), epoch.tree_hash(), identifier, next)?;
        self.upload_verified_epoch(
            &address.id,
            VerifiedEpoch::new(epoch.id(), verified.revision(), verified.signed_at()),
            user_key,
        )
        .await?;
        Ok(true)
    }

    async fn audit_address_without_keys(
        &self,
        epoch: &Epoch,
        address: &AuditedAddress,
    ) -> Result<()> {
        let identifier = Identifier::address(&address.email)?;
        let current = address.signed_key_list.as_ref();
        let revision = current.map_or(1, SignedKeyList::revision);
        let proof = routes::proof(self.api(), epoch.id(), &identifier, revision).await?;

        let Some(current) = current else {
            proof.verify_absence_of_all(self.realm(), epoch.tree_hash(), &identifier)?;
            return Ok(());
        };
        let token = current
            .obsolescence_token()
            .ok_or(Failure::ExpectedObsolescence)?;
        if !current.is_included() {
            let expected = current
                .expected_min_epoch_id()
                .filter(|_| revision != 0)
                .ok_or(Failure::MissingExpectedMinEpoch)?;
            self.add_pending(
                PendingKeyList::new(self.now(), &address.email, token, revision, expected)
                    .with_address(address.id.clone()),
            );
            return Ok(());
        }
        proof.verify_obsolescence(self.realm(), epoch.tree_hash(), &identifier, current)?;

        let next = revision.saturating_add(1);
        let next_proof = routes::proof(self.api(), epoch.id(), &identifier, next).await?;
        next_proof.verify_absence(self.realm(), epoch.tree_hash(), &identifier, next)?;
        Ok(())
    }

    async fn publish_initial_key_list(
        &self,
        epoch: &Epoch,
        address: &AuditedAddress,
        own_keys: &[ActiveKey],
    ) -> Result<()> {
        let identifier = Identifier::address(&address.email)?;
        let proof = routes::proof(self.api(), epoch.id(), &identifier, 1).await?;
        proof.verify_absence_of_all(self.realm(), epoch.tree_hash(), &identifier)?;

        let key_list =
            SignedKeyList::from_active_keys(self.pgp(), own_keys).map_err(|error| match error {
                AccountError::KeyListWithoutPrimary => Error::Failed(Failure::NoPrimaryKey),
                error => Error::Account(error),
            })?;
        let (Some(data), Some(signature)) = (key_list.data(), key_list.signature()) else {
            return Err(Failure::NoPrimaryKey.into());
        };
        let data = data.to_owned();
        let published =
            routes::publish_signed_key_list(self.api(), &address.id, &data, signature).await?;
        let expected = published
            .expected_min_epoch_id()
            .ok_or(Failure::MissingExpectedMinEpoch)?;
        self.add_pending(
            PendingKeyList::new(
                self.now(),
                &address.email,
                data,
                published.revision(),
                expected,
            )
            .with_address(address.id.clone()),
        );
        Ok(())
    }

    async fn resign_future_signature(
        &self,
        address: &AuditedAddress,
        current: &SignedKeyList,
        own_keys: &[ActiveKey],
    ) -> Result<()> {
        let primary = own_keys
            .iter()
            .find(|own| own.is_primary())
            .or_else(|| own_keys.first())
            .ok_or(Failure::NoPrimaryKey)?;
        if primary.key().version() != KEY_VERSION_4 {
            return Err(Failure::UnexpectedKeyVersion.into());
        }
        if current.data().is_none() || current.signature().is_none() {
            return Err(Failure::UnverifiedSignature.into());
        }
        let signed_in_future = current.revision() == 1
            && current
                .signed_at(
                    self.pgp(),
                    &[primary.key().public_key()],
                    LATEST_SIGNATURE_TIME,
                )
                .is_some();
        if !signed_in_future {
            return Err(Failure::UnverifiedSignature.into());
        }
        let signature =
            current.resign(self.pgp(), std::slice::from_ref(primary.key()), self.now())?;
        routes::replace_signature(self.api(), &address.id, current.revision(), &signature).await
    }

    pub(crate) async fn upload_verified_epoch(
        &self,
        address_id: &AddressId,
        verified_epoch: VerifiedEpoch,
        user_key: &PrivateKey,
    ) -> Result<()> {
        let signed = verified_epoch.sign(self.pgp(), user_key)?;
        routes::upload_verified_epoch(self.api(), address_id, &signed).await
    }
}

fn own_active_keys(keys: &AccountKeys, address: &AuditedAddress) -> Vec<ActiveKey> {
    address
        .keys
        .iter()
        .filter_map(|key| {
            keys.address_key(&key.id).map(|private| {
                ActiveKey::new(
                    private.clone(),
                    AddressKeyFlags::from_bits(key.flags),
                    key.primary,
                )
            })
        })
        .collect()
}

fn check_listed_keys(data: &str, own_keys: &[ActiveKey]) -> Result<()> {
    let listed: Vec<SignedKeyListItem> = own_keys
        .iter()
        .filter(|own| !own.key().is_forwarding_key())
        .map(|own| SignedKeyListItem::new(&own.key().public_key(), own.is_primary(), own.flags()))
        .collect();
    Ok(failure::parse_key_list(data)?
        .compare(&listed)
        .map_err(Failure::from)?)
}
