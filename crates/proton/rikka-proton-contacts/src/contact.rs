use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::card::{CardType, ContactCard};
use crate::contact_email::ContactEmail;
use crate::identifier::{ContactId, GroupId};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    #[serde(rename = "ID", default)]
    id: ContactId,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "UID", default)]
    uid: String,
    #[serde(rename = "Size", default)]
    size: u64,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(
        rename = "ModifyTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    modify_time: Timestamp,
    #[serde(rename = "ContactEmails", default)]
    emails: Vec<ContactEmail>,
    #[serde(rename = "LabelIDs", default)]
    groups: Vec<GroupId>,
    #[serde(rename = "Cards", default, skip_serializing_if = "Vec::is_empty")]
    cards: Vec<ContactCard>,
}

impl Contact {
    #[must_use]
    pub fn id(&self) -> &ContactId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn uid(&self) -> &str {
        &self.uid
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn modify_time(&self) -> Timestamp {
        self.modify_time
    }

    #[must_use]
    pub fn emails(&self) -> &[ContactEmail] {
        &self.emails
    }

    #[must_use]
    pub fn groups(&self) -> &[GroupId] {
        &self.groups
    }

    #[must_use]
    pub fn cards(&self) -> &[ContactCard] {
        &self.cards
    }

    #[must_use]
    pub fn card(&self, kind: CardType) -> Option<&ContactCard> {
        self.cards.iter().find(|card| card.kind() == kind)
    }

    #[must_use]
    pub fn has_cards(&self) -> bool {
        !self.cards.is_empty()
    }
}
