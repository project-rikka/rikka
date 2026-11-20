use rikka_proton_mail::MailAddress;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub address: String,
}

impl Person {
    #[must_use]
    pub fn new(address: impl Into<String>, name: impl Into<String>) -> Person {
        Person {
            name: name.into(),
            address: address.into(),
        }
    }

    #[must_use]
    pub fn from_address(address: &MailAddress) -> Person {
        Person::new(address.address(), address.name())
    }

    #[must_use]
    pub fn shown(&self) -> &str {
        match self.name.trim().is_empty() {
            true => self.address.as_str(),
            false => self.name.as_str(),
        }
    }

    #[must_use]
    pub fn formatted(&self) -> String {
        match self.name.trim().is_empty() {
            true => self.address.clone(),
            false => format!("{} <{}>", self.name, self.address),
        }
    }
}

impl From<&MailAddress> for Person {
    fn from(address: &MailAddress) -> Person {
        Person::from_address(address)
    }
}
