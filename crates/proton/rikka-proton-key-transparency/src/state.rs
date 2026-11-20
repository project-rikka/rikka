use rikka_proton_account::{AddressId, KeyList};
use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey};
use serde::{Deserialize, Serialize};

use crate::audit::AuditReport;
use crate::error::{Error, Result};
use crate::identifier::Scope;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditState {
    #[serde(default)]
    pending: Vec<PendingKeyList>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_audit: Option<AuditReport>,
}

impl AuditState {
    #[must_use]
    pub fn new() -> AuditState {
        AuditState::default()
    }

    #[must_use]
    pub fn pending(&self) -> &[PendingKeyList] {
        &self.pending
    }

    #[must_use]
    pub fn last_audit(&self) -> Option<&AuditReport> {
        self.last_audit.as_ref()
    }

    #[must_use]
    pub fn is_audit_due(&self, now: u64) -> bool {
        self.last_audit
            .as_ref()
            .is_none_or(|report| now >= report.next_audit_at())
    }

    pub fn seal(&self, pgp: &Pgp, user_key: &PrivateKey) -> Result<String> {
        let json = serde_json::to_vec(self).map_err(Error::State)?;
        Ok(pgp
            .encrypt()
            .to(user_key.public_key())
            .signed_by(user_key.clone())
            .armored_message(json)?)
    }

    pub fn open(armored: &str, pgp: &Pgp, user_keys: &KeyRing) -> Result<AuditState> {
        let json = pgp
            .decrypt()
            .with_ring(user_keys)
            .verified_by_ring(user_keys)
            .message(armored)?
            .into_verified()?;
        serde_json::from_slice(&json).map_err(Error::State)
    }

    pub(crate) fn add_pending(&mut self, pending: PendingKeyList) -> bool {
        let is_known = self.pending.iter().any(|pending_list| {
            pending_list.email == pending.email
                && pending_list.data == pending.data
                && pending_list.revision == pending.revision
        });
        if !is_known {
            self.pending.push(pending);
        }
        !is_known
    }

    pub(crate) fn remove_pending(&mut self, resolved: &PendingKeyList) {
        self.pending.retain(|pending_list| pending_list != resolved);
    }

    pub(crate) fn set_last_audit(&mut self, report: AuditReport) {
        self.last_audit = Some(report);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingKeyList {
    created_at: u64,
    expected_min_epoch_id: u32,
    email: String,
    data: String,
    revision: u32,
    #[serde(default)]
    catch_all: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    address_id: Option<AddressId>,
}

impl PendingKeyList {
    pub(crate) fn new(
        created_at: u64,
        email: impl Into<String>,
        content: impl Into<String>,
        revision: u32,
        expected_min_epoch_id: u32,
    ) -> PendingKeyList {
        PendingKeyList {
            created_at,
            expected_min_epoch_id,
            email: email.into(),
            data: content.into(),
            revision,
            catch_all: false,
            address_id: None,
        }
    }

    pub(crate) fn with_scope(mut self, scope: Scope) -> PendingKeyList {
        self.catch_all = scope == Scope::CatchAll;
        self
    }

    pub(crate) fn with_address(mut self, address_id: AddressId) -> PendingKeyList {
        self.address_id = Some(address_id);
        self
    }

    #[must_use]
    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    #[must_use]
    pub fn expected_min_epoch_id(&self) -> u32 {
        self.expected_min_epoch_id
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.data
    }

    #[must_use]
    pub fn revision(&self) -> u32 {
        self.revision
    }

    #[must_use]
    pub fn is_catch_all(&self) -> bool {
        self.catch_all
    }

    #[must_use]
    pub fn address_id(&self) -> Option<&AddressId> {
        self.address_id.as_ref()
    }

    pub(crate) fn scope(&self) -> Scope {
        match self.catch_all {
            true => Scope::CatchAll,
            false => Scope::Address,
        }
    }

    pub(crate) fn primary_fingerprint(&self) -> Option<String> {
        KeyList::parse(&self.data)
            .ok()
            .and_then(|list| list.primary_fingerprint().map(str::to_owned))
    }
}
