use jiff::Timestamp;
use rikka_proton_account::AddressId;
use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

use crate::identifier::CustomEmailId;

#[derive(Debug, Clone, Deserialize)]
pub struct BreachDomain {
    #[serde(rename = "Domain", default)]
    domain: String,
    #[serde(
        rename = "BreachTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    breach_time: Timestamp,
}

impl BreachDomain {
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    #[must_use]
    pub fn breach_time(&self) -> Timestamp {
        self.breach_time
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonitoredEmail {
    #[serde(rename = "AddressID", default)]
    address_id: Option<AddressId>,
    #[serde(rename = "CustomEmailID", default)]
    custom_email_id: Option<CustomEmailId>,
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "Verified", default)]
    verified: bool,
    #[serde(rename = "BreachCounter", default)]
    breach_counter: i64,
    #[serde(rename = "Flags", default)]
    flags: i64,
    #[serde(
        rename = "LastBreachTime",
        default,
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    last_breach_time: Option<Timestamp>,
    #[serde(
        rename = "LastCheckTime",
        default,
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    last_check_time: Option<Timestamp>,
}

const MONITORING_DISABLED: i64 = 1;

impl MonitoredEmail {
    #[must_use]
    pub fn address_id(&self) -> Option<&AddressId> {
        self.address_id.as_ref()
    }

    #[must_use]
    pub fn custom_email_id(&self) -> Option<&CustomEmailId> {
        self.custom_email_id.as_ref()
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.verified
    }

    #[must_use]
    pub fn breach_counter(&self) -> i64 {
        self.breach_counter
    }

    #[must_use]
    pub fn flags(&self) -> i64 {
        self.flags
    }

    #[must_use]
    pub fn last_breach_time(&self) -> Option<Timestamp> {
        self.last_breach_time
    }

    #[must_use]
    pub fn last_check_time(&self) -> Option<Timestamp> {
        self.last_check_time
    }

    #[must_use]
    pub fn is_monitored(&self) -> bool {
        self.flags & MONITORING_DISABLED == 0
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BreachSummary {
    #[serde(rename = "EmailsCount", default)]
    emails_count: i64,
    #[serde(rename = "DomainsPeek", default)]
    domains: Vec<BreachDomain>,
    #[serde(rename = "CustomEmails", default)]
    custom_emails: Vec<MonitoredEmail>,
    #[serde(rename = "Addresses", default)]
    addresses: Vec<MonitoredEmail>,
    #[serde(rename = "HasCustomDomains", default)]
    has_custom_domains: bool,
}

impl BreachSummary {
    #[must_use]
    pub fn emails_count(&self) -> i64 {
        self.emails_count
    }

    #[must_use]
    pub fn domains(&self) -> &[BreachDomain] {
        &self.domains
    }

    #[must_use]
    pub fn custom_emails(&self) -> &[MonitoredEmail] {
        &self.custom_emails
    }

    #[must_use]
    pub fn addresses(&self) -> &[MonitoredEmail] {
        &self.addresses
    }

    #[must_use]
    pub fn has_custom_domains(&self) -> bool {
        self.has_custom_domains
    }
}

wire_enum! {
    pub enum BreachState: default Unread {
        Unread = 1,
        Read = 2,
        Resolved = 3,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BreachSource {
    #[serde(rename = "IsAggregated", default)]
    aggregated: bool,
    #[serde(rename = "Domain", default)]
    domain: Option<String>,
}

impl BreachSource {
    #[must_use]
    pub fn is_aggregated(&self) -> bool {
        self.aggregated
    }

    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExposedData {
    #[serde(rename = "Code", default)]
    code: String,
    #[serde(rename = "Name", default)]
    name: String,
}

impl ExposedData {
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Breach {
    #[serde(rename = "ID", default)]
    id: String,
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "ResolvedState", default)]
    state: BreachState,
    #[serde(rename = "Severity", default)]
    severity: f64,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "PublishedAt", default)]
    published_at: Option<String>,
    #[serde(rename = "Source", default)]
    source: Option<BreachSource>,
    #[serde(rename = "Size", default)]
    size: Option<i64>,
    #[serde(rename = "ExposedData", default)]
    exposed: Vec<ExposedData>,
    #[serde(rename = "PasswordLastChars", default)]
    password_last_chars: Option<String>,
}

impl Breach {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn state(&self) -> BreachState {
        self.state
    }

    #[must_use]
    pub fn is_resolved(&self) -> bool {
        self.state == BreachState::Resolved
    }

    #[must_use]
    pub fn severity(&self) -> f64 {
        self.severity
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn published_at(&self) -> Option<&str> {
        self.published_at.as_deref()
    }

    #[must_use]
    pub fn source(&self) -> Option<&BreachSource> {
        self.source.as_ref()
    }

    #[must_use]
    pub fn size(&self) -> Option<i64> {
        self.size
    }

    #[must_use]
    pub fn exposed(&self) -> &[ExposedData] {
        &self.exposed
    }

    #[must_use]
    pub fn password_last_chars(&self) -> Option<&str> {
        self.password_last_chars.as_deref()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmailBreaches {
    #[serde(rename = "IsEligible", default)]
    eligible: bool,
    #[serde(rename = "Count", default)]
    count: i64,
    #[serde(rename = "Breaches", default)]
    breaches: Vec<Breach>,
}

impl EmailBreaches {
    #[must_use]
    pub fn is_eligible(&self) -> bool {
        self.eligible
    }

    #[must_use]
    pub fn count(&self) -> i64 {
        self.count
    }

    #[must_use]
    pub fn breaches(&self) -> &[Breach] {
        &self.breaches
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct CustomEmailRequest<'a> {
    #[serde(rename = "Email")]
    pub(crate) email: &'a str,
}

#[derive(Debug, Serialize)]
pub(crate) struct MonitorRequest {
    #[serde(rename = "Monitor")]
    pub(crate) monitor: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserMonitorRequest {
    #[serde(rename = "ProtonAddress", skip_serializing_if = "Option::is_none")]
    pub(crate) proton_addresses: Option<bool>,
    #[serde(rename = "Aliases", skip_serializing_if = "Option::is_none")]
    pub(crate) aliases: Option<bool>,
}
