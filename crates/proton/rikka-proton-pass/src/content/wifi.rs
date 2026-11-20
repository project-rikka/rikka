use crate::content::custom::CustomSection;
use crate::content::non_empty;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum WifiSecurity {
    Unspecified = 0,
    Wpa = 1,
    Wpa2 = 2,
    Wpa3 = 3,
    Wep = 4,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ItemWifi {
    #[prost(string, tag = "1")]
    pub ssid: String,
    #[prost(string, tag = "2")]
    pub password: String,
    #[prost(enumeration = "WifiSecurity", tag = "3")]
    pub security: i32,
    #[prost(message, repeated, tag = "4")]
    pub sections: Vec<CustomSection>,
}

impl WifiSecurity {
    pub fn as_str(&self) -> &'static str {
        match self {
            WifiSecurity::Unspecified => "",
            WifiSecurity::Wpa => "WPA",
            WifiSecurity::Wpa2 => "WPA2",
            WifiSecurity::Wpa3 => "WPA3",
            WifiSecurity::Wep => "WEP",
        }
    }
}

impl ItemWifi {
    pub fn network(&self) -> Option<&str> {
        non_empty(&self.ssid)
    }
}
