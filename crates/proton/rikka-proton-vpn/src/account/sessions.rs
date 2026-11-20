use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct VpnSession {
    #[serde(rename = "SessionID")]
    pub id: String,
    #[serde(rename = "ExitIP", default)]
    pub exit_ip: String,
    #[serde(rename = "Protocol", default)]
    pub protocol: Option<String>,
}
