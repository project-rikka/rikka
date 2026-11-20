use jiff::Timestamp;
use std::fmt;

use rikka_proton_pass::{ExtraValue, Item, ItemId, ItemIdentity, ItemState, PassItem, ShareId};

use crate::item_type::ItemType;
use crate::strength::Strength;
use crate::values::{Card, Field, Section, Ssh, Value, Wifi};

#[derive(Clone)]
pub struct OpenedItem {
    pub share: ShareId,
    pub id: ItemId,
    pub revision: i64,
    pub item_type: ItemType,
    pub title: String,
    pub note: String,
    pub pinned: bool,
    pub trashed: bool,
    pub created: Timestamp,
    pub modified: Timestamp,
    pub used: Timestamp,
    pub email: String,
    pub username: String,
    pub password: String,
    pub strength: Option<Strength>,
    pub urls: Vec<String>,
    pub totp: String,
    pub passkeys: usize,
    pub alias: String,
    pub card: Card,
    pub wifi: Wifi,
    pub ssh: Ssh,
    pub fields: Vec<Field>,
    pub sections: Vec<Section>,
    pub raw: Item,
}

impl OpenedItem {
    #[must_use]
    pub fn from_item(item: &PassItem) -> OpenedItem {
        let content = item.content();
        let login = content.login();
        let card = content.credit_card();
        let wifi = content.wifi();
        let ssh = content.ssh_key();

        let fields: Vec<Field> = content.extra_fields.iter().map(Field::from_extra).collect();
        let sections: Vec<Section> = content
            .sections()
            .iter()
            .map(|section| Section {
                name: section.section_name.clone(),
                fields: section
                    .section_fields
                    .iter()
                    .map(Field::from_extra)
                    .collect(),
            })
            .collect();

        OpenedItem {
            share: item.share_id().clone(),
            id: item.id().clone(),
            revision: item.revision(),
            item_type: ItemType::from_kind(item.kind()),
            title: item.name().to_owned(),
            note: item.note().unwrap_or_default().to_owned(),
            pinned: item.is_pinned(),
            trashed: item.state() == ItemState::Trashed,
            created: item.response().create_time(),
            modified: item.response().modify_time(),
            used: item.response().last_use_time().unwrap_or_default(),
            email: login
                .map(|login| login.item_email.clone())
                .unwrap_or_default(),
            username: login
                .map(|login| login.item_username.clone())
                .unwrap_or_default(),
            password: login
                .map(|login| login.password.clone())
                .unwrap_or_default(),
            strength: login
                .map(|login| login.password.as_str())
                .filter(|password| !password.is_empty())
                .map(Strength::estimate),
            urls: login.map(|login| login.urls.clone()).unwrap_or_default(),
            totp: login
                .map(|login| login.totp_uri.clone())
                .unwrap_or_default(),
            passkeys: login.map_or(0, |login| login.passkeys.len()),
            alias: item.alias_email().unwrap_or_default().to_owned(),
            card: card.map_or_else(Card::default, |card| Card {
                holder: card.cardholder_name.clone(),
                number: card.number.clone(),
                verification: card.verification_number.clone(),
                expiration: card.expiration_date.clone(),
                pin: card.pin.clone(),
                kind: card.card_type,
            }),
            wifi: wifi.map_or_else(Wifi::default, |wifi| Wifi {
                ssid: wifi.ssid.clone(),
                password: wifi.password.clone(),
                security: wifi.security,
            }),
            ssh: ssh.map_or_else(Ssh::default, |key| Ssh {
                public: key.public_key.clone(),
                private: key.private_key.clone(),
            }),
            fields,
            sections,
            raw: content.clone(),
        }
    }

    #[must_use]
    pub fn identifier(&self) -> &str {
        match self.item_type {
            ItemType::Alias => self.alias.as_str(),
            _ => match self.username.is_empty() {
                true => self.email.as_str(),
                false => self.username.as_str(),
            },
        }
    }

    #[must_use]
    pub fn identity(&self) -> Option<&ItemIdentity> {
        self.raw.identity()
    }

    #[must_use]
    pub fn has_codes(&self) -> bool {
        self.codes().next().is_some()
    }

    pub fn codes(&self) -> impl Iterator<Item = (&str, &str)> {
        let primary = (!self.totp.is_empty()).then_some(("", self.totp.as_str()));
        let custom = self
            .fields
            .iter()
            .chain(self.sections.iter().flat_map(|section| &section.fields))
            .filter_map(|field| match &field.value {
                Value::Code(uri) if !uri.is_empty() => Some((field.name.as_str(), uri.as_str())),
                _ => None,
            });
        let details = self
            .identity()
            .into_iter()
            .flat_map(ItemIdentity::details)
            .filter_map(|field| match field.value() {
                Some(ExtraValue::Totp(uri)) if !uri.is_empty() => Some((field.name(), uri)),
                _ => None,
            });

        primary.into_iter().chain(custom).chain(details)
    }
}

impl fmt::Debug for OpenedItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenedItem")
            .field("share", &self.share)
            .field("id", &self.id)
            .field("revision", &self.revision)
            .field("item_type", &self.item_type)
            .finish_non_exhaustive()
    }
}
