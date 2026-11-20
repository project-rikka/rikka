use futures_util::future::try_join_all;
use rikka_proton_account::{AccountKeys, AddressId, SignedKeyList};
use rikka_proton_pgp::{KeyRing, PrivateKey, PublicKey};

use crate::client::{EpochCache, KeyTransparency};
use crate::error::{Error, Result};
use crate::failure::{self, Failure};
use crate::identifier::Identifier;
use crate::outbound::failure_into_verdict;
use crate::routes;
use crate::state::PendingKeyList;
use crate::verdict::{Unverifiable, Verdict};
use crate::verified_epoch::VerifiedEpoch;

impl KeyTransparency {
    pub async fn record_published(
        &self,
        address_id: &AddressId,
        email: &str,
        previous_revision: Option<u32>,
        data: &str,
    ) -> Result<()> {
        self.ensure_available().await?;
        let created_at = self.now();
        let identifier = Identifier::address(email)?;
        let revisions =
            routes::signed_key_lists(self.api(), previous_revision.unwrap_or(0), &identifier)
                .await?;
        let published = revisions
            .iter()
            .find(|list| list.data() == Some(data) && list.revision() != 0)
            .ok_or(Failure::PublishedKeyListNotFound)?;
        let expected = published
            .expected_min_epoch_id()
            .ok_or(Failure::PublishedKeyListNotFound)?;
        self.add_pending(
            PendingKeyList::new(created_at, email, data, published.revision(), expected)
                .with_address(address_id.clone()),
        );
        Ok(())
    }

    pub async fn reset_audit(&self, keys: &AccountKeys) -> Result<()> {
        self.ensure_available().await?;
        let user_key = keys.primary_user_key().ok_or(Error::NoUserKey)?;
        let epoch = &self.latest(EpochCache::Refresh).await?;
        try_join_all(keys.addresses().iter().map(|address| async move {
            let address = routes::address(self.api(), address.id()).await?;
            let revision = address
                .signed_key_list
                .as_ref()
                .map_or(0, SignedKeyList::revision);
            if address.is_disabled() || revision == 0 {
                return Ok(());
            }
            self.upload_verified_epoch(
                &address.id,
                VerifiedEpoch::new(epoch.id(), revision, 0),
                user_key,
            )
            .await
        }))
        .await?;
        Ok(())
    }

    pub async fn resign_history(
        &self,
        address_id: &AddressId,
        email: &str,
        new_primary_keys: &[PrivateKey],
        former_primary_keys: &[PublicKey],
        user_keys: &KeyRing,
    ) -> Result<()> {
        self.ensure_available().await?;
        let identifier = Identifier::address(email)?;
        let verified_epoch = routes::verified_epoch(self.api(), address_id)
            .await?
            .and_then(|signed| signed.verify(self.pgp(), user_keys));
        let revisions = routes::signed_key_lists(
            self.api(),
            verified_epoch.map_or(0, |verified| verified.revision()),
            &identifier,
        )
        .await?;

        for list in &revisions {
            if list.data().is_none() || list.signature().is_none() {
                continue;
            }
            let Some(signed_at) = list.signed_at(self.pgp(), former_primary_keys, self.now())
            else {
                continue;
            };
            if new_primary_keys
                .iter()
                .any(|key| key.public_key().created_at() > signed_at)
            {
                continue;
            }
            let signature = list.resign(self.pgp(), new_primary_keys, signed_at)?;
            routes::replace_signature(self.api(), address_id, list.revision(), &signature).await?;
        }
        Ok(())
    }

    pub async fn verify_absent_or_obsolete(
        &self,
        email: &str,
        signed_key_list: Option<&SignedKeyList>,
    ) -> Result<Verdict> {
        failure_into_verdict(self.evaluate_migration(email, signed_key_list).await)
    }

    pub async fn verify_recovered_key(
        &self,
        email: &str,
        key: &PublicKey,
        signed_key_list: Option<&SignedKeyList>,
    ) -> Result<Verdict> {
        failure_into_verdict(self.evaluate_recovery(email, key, signed_key_list).await)
    }

    async fn evaluate_migration(
        &self,
        email: &str,
        signed_key_list: Option<&SignedKeyList>,
    ) -> Result<Verdict> {
        let epoch = self.latest(EpochCache::Refresh).await?;
        let identifier = Identifier::address(email)?;
        match signed_key_list.filter(|list| list.obsolescence_token().is_some()) {
            Some(list) => {
                let revision = match list.revision() {
                    0 => return Err(Failure::IncompleteSignedKeyList.into()),
                    revision => revision,
                };
                let next = revision.saturating_add(1);
                let proofs = self.proofs(&epoch, &identifier, [revision, next]).await?;
                let [proof, next_proof] = proofs.as_slice() else {
                    return Err(Failure::IncompleteSignedKeyList.into());
                };
                proof.verify_obsolescence(self.realm(), epoch.tree_hash(), &identifier, list)?;
                next_proof.verify_absence(self.realm(), epoch.tree_hash(), &identifier, next)?;
                Ok(Verdict::Unverifiable(Unverifiable::Obsolete))
            }
            None => {
                let proof = routes::proof(self.api(), epoch.id(), &identifier, 1).await?;
                proof.verify_absence_of_all(self.realm(), epoch.tree_hash(), &identifier)?;
                Ok(Verdict::Unverifiable(Unverifiable::Absent))
            }
        }
    }

    async fn evaluate_recovery(
        &self,
        email: &str,
        key: &PublicKey,
        signed_key_list: Option<&SignedKeyList>,
    ) -> Result<Verdict> {
        self.ensure_available().await?;
        let Some(list) = signed_key_list else {
            return Ok(Verdict::Unverifiable(Unverifiable::NoSignedKeyList));
        };
        let (Some(data), Some(_)) = (list.data(), list.signature()) else {
            return Ok(Verdict::Unverifiable(Unverifiable::Obsolete));
        };
        let Some(max_epoch_id) = list.max_epoch_id() else {
            return Ok(Verdict::Unverifiable(Unverifiable::NeverIncluded));
        };

        let key_list = failure::parse_key_list(data)?;
        let item = key_list.find(key).ok_or(Failure::KeyNotListed)?;
        if item.is_compromised() {
            return Err(Failure::KeyCompromised.into());
        }
        list.signed_at(self.pgp(), std::slice::from_ref(key), self.now())
            .ok_or(Failure::UnverifiedSignature)?;

        let identifier = Identifier::address(email)?;
        let proof = routes::proof(self.api(), max_epoch_id, &identifier, list.revision()).await?;
        let epoch = self.epoch(max_epoch_id).await?;
        proof.verify_existence(self.realm(), epoch.tree_hash(), &identifier, list)?;
        Ok(Verdict::Verified)
    }
}
