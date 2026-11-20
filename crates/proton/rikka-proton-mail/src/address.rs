use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MailAddress {
    #[serde(rename = "Address")]
    address: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Group", default, skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    #[serde(
        rename = "IsProton",
        default,
        skip_serializing,
        deserialize_with = "rikka_proton_api::flag::deserialize"
    )]
    proton: bool,
}

impl MailAddress {
    #[must_use]
    pub fn new(address: impl Into<String>) -> MailAddress {
        MailAddress {
            address: address.into(),
            name: String::new(),
            group: None,
            proton: false,
        }
    }

    #[must_use]
    pub fn with_name(address: impl Into<String>, name: impl Into<String>) -> MailAddress {
        MailAddress {
            address: address.into(),
            name: name.into(),
            group: None,
            proton: false,
        }
    }

    #[must_use]
    pub fn in_group(mut self, group: impl Into<String>) -> MailAddress {
        self.group = Some(group.into());
        self
    }

    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    #[must_use]
    pub fn is_proton(&self) -> bool {
        self.proton
    }
}

impl From<&str> for MailAddress {
    fn from(address: &str) -> MailAddress {
        MailAddress::new(address)
    }
}
