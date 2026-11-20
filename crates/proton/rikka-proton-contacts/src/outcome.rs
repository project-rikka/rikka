use serde::Deserialize;

use crate::contact::Contact;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContactReply {
    #[serde(rename = "Contact", default)]
    contact: Option<Contact>,
}

impl ContactReply {
    #[must_use]
    pub fn contact(&self) -> Option<&Contact> {
        self.contact.as_ref()
    }
}
