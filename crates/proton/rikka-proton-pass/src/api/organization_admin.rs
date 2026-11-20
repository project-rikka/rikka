use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::api::{
    AliasCreateMode, ExportMode, ItemShareMode, SecureLinkMode, ShareMode, VaultCreateMode,
};
use crate::identifier::PausedUrlId;

#[must_use]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct OrganizationUpdate {
    #[serde(rename = "ShareMode", skip_serializing_if = "Option::is_none")]
    share_mode: Option<ShareMode>,
    #[serde(rename = "ShareAcceptMode", skip_serializing_if = "Option::is_none")]
    share_accept_mode: Option<ShareMode>,
    #[serde(rename = "ItemShareMode", skip_serializing_if = "Option::is_none")]
    item_share_mode: Option<ItemShareMode>,
    #[serde(rename = "PublicLinkMode", skip_serializing_if = "Option::is_none")]
    secure_link_mode: Option<SecureLinkMode>,
    #[serde(rename = "ForceLockSeconds", skip_serializing_if = "Option::is_none")]
    force_lock_seconds: Option<u64>,
    #[serde(rename = "ExportMode", skip_serializing_if = "Option::is_none")]
    export_mode: Option<ExportMode>,
    #[serde(rename = "VaultCreateMode", skip_serializing_if = "Option::is_none")]
    vault_create_mode: Option<VaultCreateMode>,
    #[serde(rename = "AliasCreateMode", skip_serializing_if = "Option::is_none")]
    alias_create_mode: Option<AliasCreateMode>,
}

impl OrganizationUpdate {
    pub fn new() -> OrganizationUpdate {
        OrganizationUpdate::default()
    }

    pub fn share_mode(mut self, mode: ShareMode) -> OrganizationUpdate {
        self.share_mode = Some(mode);
        self
    }

    pub fn share_accept_mode(mut self, mode: ShareMode) -> OrganizationUpdate {
        self.share_accept_mode = Some(mode);
        self
    }

    pub fn item_share_mode(mut self, mode: ItemShareMode) -> OrganizationUpdate {
        self.item_share_mode = Some(mode);
        self
    }

    pub fn secure_link_mode(mut self, mode: SecureLinkMode) -> OrganizationUpdate {
        self.secure_link_mode = Some(mode);
        self
    }

    pub fn force_lock_seconds(mut self, seconds: u64) -> OrganizationUpdate {
        self.force_lock_seconds = Some(seconds);
        self
    }

    pub fn export_mode(mut self, mode: ExportMode) -> OrganizationUpdate {
        self.export_mode = Some(mode);
        self
    }

    pub fn vault_create_mode(mut self, mode: VaultCreateMode) -> OrganizationUpdate {
        self.vault_create_mode = Some(mode);
        self
    }

    pub fn alias_create_mode(mut self, mode: AliasCreateMode) -> OrganizationUpdate {
        self.alias_create_mode = Some(mode);
        self
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreachCounter {
    #[serde(rename = "BreachEmailCount", default)]
    emails: i64,
    #[serde(rename = "TotalBreachCount", default)]
    breaches: i64,
}

impl BreachCounter {
    #[must_use]
    pub fn emails(&self) -> i64 {
        self.emails
    }

    #[must_use]
    pub fn breaches(&self) -> i64 {
        self.breaches
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberItemsReport {
    #[serde(rename = "OwnedVaultCount", default)]
    owned_vaults: i64,
    #[serde(rename = "OwnedItemCount", default)]
    owned_items: i64,
    #[serde(rename = "AccessibleItemCount", default)]
    accessible_items: i64,
    #[serde(rename = "AccessibleVaultCount", default)]
    accessible_vaults: i64,
}

impl MemberItemsReport {
    #[must_use]
    pub fn owned_vaults(&self) -> i64 {
        self.owned_vaults
    }

    #[must_use]
    pub fn owned_items(&self) -> i64 {
        self.owned_items
    }

    #[must_use]
    pub fn accessible_items(&self) -> i64 {
        self.accessible_items
    }

    #[must_use]
    pub fn accessible_vaults(&self) -> i64 {
        self.accessible_vaults
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorReport {
    #[serde(rename = "ReusedPasswords", default)]
    reused_passwords: u32,
    #[serde(rename = "Inactive2FA", default)]
    inactive_two_factor: u32,
    #[serde(rename = "ExcludedItems", default)]
    excluded_items: u32,
    #[serde(rename = "WeakPasswords", default)]
    weak_passwords: u32,
    #[serde(rename = "CompromisedPasswords", default)]
    compromised_passwords: u32,
}

impl MonitorReport {
    #[must_use]
    pub fn new() -> MonitorReport {
        MonitorReport::default()
    }

    #[must_use]
    pub fn with_reused_passwords(mut self, count: u32) -> MonitorReport {
        self.reused_passwords = count;
        self
    }

    #[must_use]
    pub fn with_inactive_two_factor(mut self, count: u32) -> MonitorReport {
        self.inactive_two_factor = count;
        self
    }

    #[must_use]
    pub fn with_excluded_items(mut self, count: u32) -> MonitorReport {
        self.excluded_items = count;
        self
    }

    #[must_use]
    pub fn with_weak_passwords(mut self, count: u32) -> MonitorReport {
        self.weak_passwords = count;
        self
    }

    #[must_use]
    pub fn with_compromised_passwords(mut self, count: u32) -> MonitorReport {
        self.compromised_passwords = count;
        self
    }

    #[must_use]
    pub fn reused_passwords(&self) -> u32 {
        self.reused_passwords
    }

    #[must_use]
    pub fn inactive_two_factor(&self) -> u32 {
        self.inactive_two_factor
    }

    #[must_use]
    pub fn excluded_items(&self) -> u32 {
        self.excluded_items
    }

    #[must_use]
    pub fn weak_passwords(&self) -> u32 {
        self.weak_passwords
    }

    #[must_use]
    pub fn compromised_passwords(&self) -> u32 {
        self.compromised_passwords
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMonitorReport {
    #[serde(flatten)]
    counts: MonitorReport,
    #[serde(
        rename = "ReportTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    report_time: Timestamp,
    #[serde(rename = "ClientVersion", default)]
    client_version: String,
}

impl StoredMonitorReport {
    #[must_use]
    pub fn counts(&self) -> &MonitorReport {
        &self.counts
    }

    #[must_use]
    pub fn report_time(&self) -> Timestamp {
        self.report_time
    }

    #[must_use]
    pub fn client_version(&self) -> &str {
        &self.client_version
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberReport {
    #[serde(rename = "PrimaryEmail", default)]
    primary_email: String,
    #[serde(rename = "CustomEmailBreachCount", default)]
    custom_email_breaches: BreachCounter,
    #[serde(rename = "AddressBreachCount", default)]
    address_breaches: BreachCounter,
    #[serde(rename = "ItemsReport", default)]
    items: MemberItemsReport,
    #[serde(rename = "MonitorReport", default)]
    monitor: Option<StoredMonitorReport>,
    #[serde(
        rename = "LastActivityTime",
        default,
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    last_activity_time: Option<Timestamp>,
}

impl MemberReport {
    #[must_use]
    pub fn primary_email(&self) -> &str {
        &self.primary_email
    }

    #[must_use]
    pub fn custom_email_breaches(&self) -> BreachCounter {
        self.custom_email_breaches
    }

    #[must_use]
    pub fn address_breaches(&self) -> BreachCounter {
        self.address_breaches
    }

    #[must_use]
    pub fn items(&self) -> MemberItemsReport {
        self.items
    }

    #[must_use]
    pub fn monitor(&self) -> Option<&StoredMonitorReport> {
        self.monitor.as_ref()
    }

    #[must_use]
    pub fn last_activity_time(&self) -> Option<Timestamp> {
        self.last_activity_time
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PauseValues {
    #[serde(rename = "AutofillEnabled", default)]
    pub autofill: bool,
    #[serde(rename = "Autofill2faEnabled", default)]
    pub autofill_two_factor: bool,
    #[serde(rename = "AutofillAutosuggestEnabled", default)]
    pub autosuggest: bool,
    #[serde(rename = "AutosaveEnabled", default)]
    pub autosave: bool,
    #[serde(rename = "PasskeysEnabled", default)]
    pub passkeys: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PausedUrl {
    #[serde(rename = "EntryID")]
    id: PausedUrlId,
    #[serde(rename = "Url", default)]
    url: String,
    #[serde(rename = "Values", default)]
    values: PauseValues,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
}

impl PausedUrl {
    #[must_use]
    pub fn id(&self) -> &PausedUrlId {
        &self.id
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn values(&self) -> PauseValues {
        self.values
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReportPage {
    #[serde(rename = "MemberReports", default)]
    pub(crate) members: Vec<MemberReport>,
    #[serde(rename = "TotalMemberCount", default)]
    pub(crate) total: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PausedUrlPage {
    #[serde(rename = "Entries", default)]
    pub(crate) entries: Vec<PausedUrl>,
    #[serde(rename = "LastID", default)]
    pub(crate) last_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PauseUrlRequest<'a> {
    #[serde(rename = "Url")]
    pub(crate) url: &'a str,
    #[serde(rename = "Values")]
    pub(crate) values: PauseValues,
}
