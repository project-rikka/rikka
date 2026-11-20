use crate::access_point::ApMode;
use crate::security::Security;
use crate::ssid::Ssid;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WirelessProfile {
    pub ssid: Ssid,
    pub hidden: bool,
    pub mode: ApMode,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SecurityProfile {
    pub security: Security,
    pub passphrase: Option<String>,
}
