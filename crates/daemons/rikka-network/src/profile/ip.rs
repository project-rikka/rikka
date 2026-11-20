use crate::ip::{Address, Route};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IpMethod {
    #[default]
    Automatic,
    Manual,
    LinkLocal,
    Shared,
    Disabled,
    DhcpOnly,
}

impl IpMethod {
    pub(crate) fn read(name: &str) -> Self {
        match name {
            "manual" => Self::Manual,
            "link-local" => Self::LinkLocal,
            "shared" => Self::Shared,
            "disabled" | "ignore" => Self::Disabled,
            "dhcp" => Self::DhcpOnly,
            _ => Self::Automatic,
        }
    }

    pub(crate) fn write(self) -> &'static str {
        match self {
            Self::Automatic => "auto",
            Self::Manual => "manual",
            Self::LinkLocal => "link-local",
            Self::Shared => "shared",
            Self::Disabled => "disabled",
            Self::DhcpOnly => "dhcp",
        }
    }

    #[must_use]
    pub fn is_manual(self) -> bool {
        self == Self::Manual
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IpProfile {
    pub method: IpMethod,
    pub addresses: Vec<Address>,
    pub gateway: String,
    pub nameservers: Vec<String>,
    pub searches: Vec<String>,
    pub routes: Vec<Route>,
    pub ignore_automatic_dns: bool,
    pub ignore_automatic_routes: bool,
    pub never_default: bool,
}
