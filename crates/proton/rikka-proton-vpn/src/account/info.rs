use jiff::Timestamp;
use rikka_proton_pgp::Secret;
use serde::Deserialize;

use crate::servers::Tier;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct NetShieldAvailability {
    #[serde(rename = "Malware")]
    pub malware: bool,
    #[serde(rename = "AdsAndTrackers")]
    pub ads_and_trackers: bool,
    #[serde(rename = "AdultContent")]
    pub adult_content: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VpnInfo {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Password", default)]
    pub password: Secret,
    #[serde(rename = "Status", default)]
    pub status: i32,
    #[serde(
        rename = "ExpirationTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    pub expires_at: Option<Timestamp>,
    #[serde(rename = "PlanName", default)]
    pub plan_name: Option<String>,
    #[serde(rename = "PlanTitle", default)]
    pub plan_title: Option<String>,
    #[serde(rename = "MaxTier", default)]
    pub max_tier: Option<Tier>,
    #[serde(rename = "MaxConnect", default)]
    pub max_connect: u32,
    #[serde(rename = "GroupID", default)]
    pub group_id: Option<String>,
    #[serde(rename = "Groups", default)]
    pub groups: Vec<String>,
    #[serde(rename = "NeedConnectionAllocation", default)]
    pub need_connection_allocation: bool,
    #[serde(rename = "IsBusiness", default)]
    pub is_business: bool,
    #[serde(rename = "NetShield", default)]
    pub netshield: Option<NetShieldAvailability>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VpnAccount {
    #[serde(rename = "VPN")]
    pub vpn: VpnInfo,
    #[serde(rename = "Subscribed", default)]
    pub subscribed: i32,
    #[serde(rename = "Services", default)]
    pub services: i32,
    #[serde(rename = "Delinquent", default)]
    pub delinquent: i32,
    #[serde(rename = "HasPaymentMethod", default)]
    pub has_payment_method: Option<i32>,
    #[serde(rename = "Credit", default)]
    pub credit: Option<i32>,
    #[serde(rename = "Currency", default)]
    pub currency: Option<String>,
    #[serde(rename = "Warnings", default)]
    pub warnings: Vec<String>,
}

impl VpnInfo {
    #[must_use]
    pub fn tier(&self) -> Tier {
        self.max_tier.unwrap_or(Tier::FREE)
    }

    #[must_use]
    pub fn tier_is_known(&self) -> bool {
        self.plan_name.is_some() || self.max_connect <= 1
    }
}
