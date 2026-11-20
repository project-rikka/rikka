use crate::content::non_empty;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum VaultIcon {
    Unspecified = 0,
    Custom = 1,
    Icon1 = 2,
    Icon2 = 3,
    Icon3 = 4,
    Icon4 = 5,
    Icon5 = 6,
    Icon6 = 7,
    Icon7 = 8,
    Icon8 = 9,
    Icon9 = 10,
    Icon10 = 11,
    Icon11 = 12,
    Icon12 = 13,
    Icon13 = 14,
    Icon14 = 15,
    Icon15 = 16,
    Icon16 = 17,
    Icon17 = 18,
    Icon18 = 19,
    Icon19 = 20,
    Icon20 = 21,
    Icon21 = 22,
    Icon22 = 23,
    Icon23 = 24,
    Icon24 = 25,
    Icon25 = 26,
    Icon26 = 27,
    Icon27 = 28,
    Icon28 = 29,
    Icon29 = 30,
    Icon30 = 31,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum VaultColor {
    Unspecified = 0,
    Custom = 1,
    Color1 = 2,
    Color2 = 3,
    Color3 = 4,
    Color4 = 5,
    Color5 = 6,
    Color6 = 7,
    Color7 = 8,
    Color8 = 9,
    Color9 = 10,
    Color10 = 11,
}

#[derive(Clone, Copy, PartialEq, Eq, prost::Message)]
pub struct VaultDisplayPreferences {
    #[prost(enumeration = "VaultIcon", tag = "1")]
    pub icon: i32,
    #[prost(enumeration = "VaultColor", tag = "2")]
    pub color: i32,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct Vault {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub description: String,
    #[prost(message, optional, tag = "3")]
    pub display: Option<VaultDisplayPreferences>,
}

impl Vault {
    pub fn description(&self) -> Option<&str> {
        non_empty(&self.description)
    }

    pub fn icon(&self) -> VaultIcon {
        self.display
            .map(|display| display.icon())
            .unwrap_or_default()
    }

    pub fn color(&self) -> VaultColor {
        self.display
            .map(|display| display.color())
            .unwrap_or_default()
    }
}
