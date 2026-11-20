use rikka_proton_account::SignedKeyList;
use rikka_proton_pgp::{Pgp, PublicKey};

use crate::audit::report::{AddressOutcome, AuditWarning};
use crate::failure::Failure;
use crate::identifier::Identifier;
use crate::proof::Proof;
use crate::realm::Realm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RevisionStatus {
    Deleted,
    Obsolete,
    Unverified,
    Verified { signed_at: u64 },
}

#[derive(Debug, Clone)]
pub(crate) struct Revision {
    pub(crate) revision: u32,
    pub(crate) status: RevisionStatus,
    pub(crate) data: Option<String>,
}

pub(crate) struct History {
    pub(crate) realm: Realm,
    pub(crate) pgp: Pgp,
    pub(crate) tree_hash: String,
    pub(crate) identifier: Identifier,
    pub(crate) verification_keys: Vec<PublicKey>,
    pub(crate) now: u64,
}

impl History {
    pub(crate) fn audit(
        &self,
        revision: u32,
        signed_key_list: Option<&SignedKeyList>,
        proof: &Proof,
    ) -> Result<Revision, Failure> {
        let Some(signed_key_list) = signed_key_list else {
            proof.verify_absence(&self.realm, &self.tree_hash, &self.identifier, revision)?;
            return Ok(Revision {
                revision,
                status: RevisionStatus::Deleted,
                data: None,
            });
        };
        proof.verify_inclusion(
            &self.realm,
            &self.tree_hash,
            &self.identifier,
            signed_key_list,
        )?;
        let status = match (signed_key_list.data(), signed_key_list.signature()) {
            (Some(_), Some(_)) => {
                match signed_key_list.signed_at(&self.pgp, &self.verification_keys, self.now) {
                    Some(signed_at) => RevisionStatus::Verified { signed_at },
                    None => RevisionStatus::Unverified,
                }
            }
            _ => RevisionStatus::Obsolete,
        };
        Ok(Revision {
            revision,
            status,
            data: signed_key_list.data().map(str::to_owned),
        })
    }
}

pub(crate) fn is_non_decreasing<T: Ord + Copy>(
    values: impl IntoIterator<Item = T>,
    start: T,
) -> bool {
    let mut bound = start;
    for value in values {
        if value < bound {
            return false;
        }
        bound = value;
    }
    true
}

pub(crate) fn history_outcome(
    revisions: &[Revision],
    signature_was_in_future: bool,
) -> AddressOutcome {
    let address_was_disabled = revisions
        .iter()
        .any(|revision| revision.status == RevisionStatus::Obsolete);
    let unverified = revisions
        .iter()
        .filter(|revision| revision.status == RevisionStatus::Unverified)
        .count();
    let signature_verification_failed =
        unverified > 1 || (unverified == 1 && !signature_was_in_future);
    if address_was_disabled || signature_verification_failed {
        AddressOutcome::Warned(AuditWarning::UnverifiableHistory {
            address_was_disabled,
            signature_verification_failed,
        })
    } else {
        AddressOutcome::Passed
    }
}
