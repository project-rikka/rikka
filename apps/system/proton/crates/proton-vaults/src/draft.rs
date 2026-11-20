use std::fmt;

use rikka_proton_pass::{
    Content, CustomSection, ExtraField, ExtraFieldContent, ExtraHiddenField, ExtraTextField,
    ExtraValue, Item, ItemAlias, ItemContent, ItemCreditCard, ItemCustom, ItemId, ItemIdentity,
    ItemLogin, ItemNote, ItemSshKey, ItemWifi, Metadata, ShareId,
};

use crate::item_type::ItemType;
use crate::opened::OpenedItem;
use crate::values::{Card, CustomField, Group, Ssh, Wifi};

#[derive(Clone)]
pub struct Draft {
    pub share: ShareId,
    pub id: Option<ItemId>,
    pub item_type: ItemType,
    pub title: String,
    pub note: String,
    pub email: String,
    pub username: String,
    pub password: String,
    pub totp: String,
    pub urls: String,
    pub card: Card,
    pub wifi: Wifi,
    pub ssh: Ssh,
    pub identity: ItemIdentity,
    pub fields: Vec<CustomField>,
    pub groups: Vec<Group>,
    origin: Option<Item>,
}

impl Draft {
    #[must_use]
    pub fn fresh(share: ShareId, item_type: ItemType) -> Draft {
        Draft {
            share,
            id: None,
            item_type,
            title: String::new(),
            note: String::new(),
            email: String::new(),
            username: String::new(),
            password: String::new(),
            totp: String::new(),
            urls: String::new(),
            card: Card::default(),
            wifi: Wifi::default(),
            ssh: Ssh::default(),
            identity: ItemIdentity::default(),
            fields: Vec::new(),
            groups: Vec::new(),
            origin: None,
        }
    }

    #[must_use]
    pub fn from_opened(opened: &OpenedItem) -> Draft {
        Draft {
            share: opened.share.clone(),
            id: Some(opened.id.clone()),
            item_type: opened.item_type,
            title: opened.title.clone(),
            note: opened.note.clone(),
            email: opened.email.clone(),
            username: opened.username.clone(),
            password: opened.password.clone(),
            totp: opened.totp.clone(),
            urls: opened.urls.join("\n"),
            card: opened.card.clone(),
            wifi: opened.wifi.clone(),
            ssh: opened.ssh.clone(),
            identity: opened.identity().cloned().unwrap_or_default(),
            fields: opened.raw.extra_fields.iter().map(custom_field).collect(),
            groups: opened
                .raw
                .sections()
                .iter()
                .map(|section| Group {
                    name: section.section_name.clone(),
                    fields: section.section_fields.iter().map(custom_field).collect(),
                })
                .collect(),
            origin: Some(opened.raw.clone()),
        }
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        !self.title.trim().is_empty()
    }

    #[must_use]
    pub fn is_new(&self) -> bool {
        self.id.is_none()
    }

    #[must_use]
    pub fn item(&self) -> Item {
        let mut item = self
            .origin
            .clone()
            .unwrap_or_else(|| Item::new(Metadata::default(), self.content()));

        let metadata = item.metadata.get_or_insert_default();
        metadata.name = self.title.trim().to_owned();
        metadata.note = self.note.clone();

        let content = item.content.get_or_insert_with(|| Content {
            content: Some(self.content()),
        });
        let body = content.content.get_or_insert_with(|| self.content());
        self.patch(body);

        item.extra_fields = self
            .fields
            .iter()
            .filter(|field| !field.is_empty())
            .map(extra_field)
            .collect();

        item
    }

    fn patch(&self, body: &mut ItemContent) {
        match (self.item_type, body) {
            (ItemType::Login, ItemContent::Login(login)) => {
                login.item_email = self.email.trim().to_owned();
                login.item_username = self.username.trim().to_owned();
                login.password = self.password.clone();
                login.totp_uri = self.totp.trim().to_owned();
                login.urls = lines(&self.urls);
            }
            (ItemType::Card, ItemContent::CreditCard(card)) => {
                card.cardholder_name = self.card.holder.clone();
                card.number = self.card.number.replace([' ', '-'], "");
                card.verification_number = self.card.verification.clone();
                card.expiration_date = self.card.expiration.clone();
                card.pin = self.card.pin.clone();
                card.card_type = self.card.kind;
            }
            (ItemType::Wifi, ItemContent::Wifi(wifi)) => {
                wifi.ssid = self.wifi.ssid.clone();
                wifi.password = self.wifi.password.clone();
                wifi.security = self.wifi.security;
                wifi.sections = self.sections();
            }
            (ItemType::SshKey, ItemContent::SshKey(key)) => {
                key.public_key = self.ssh.public.clone();
                key.private_key = self.ssh.private.clone();
                key.sections = self.sections();
            }
            (ItemType::Identity, ItemContent::Identity(identity)) => {
                let extras = (
                    std::mem::take(&mut identity.extra_personal_details),
                    std::mem::take(&mut identity.extra_address_details),
                    std::mem::take(&mut identity.extra_contact_details),
                    std::mem::take(&mut identity.extra_work_details),
                );
                **identity = self.identity.clone();
                identity.extra_personal_details = extras.0;
                identity.extra_address_details = extras.1;
                identity.extra_contact_details = extras.2;
                identity.extra_work_details = extras.3;
                identity.extra_sections = self.sections();
            }
            (ItemType::Custom, ItemContent::Custom(custom)) => {
                custom.sections = self.sections();
            }
            (ItemType::Alias | ItemType::Note, _) => {}
            (_, body) => *body = self.content(),
        }
    }

    fn sections(&self) -> Vec<CustomSection> {
        self.groups
            .iter()
            .filter(|group| !group.is_empty())
            .map(|group| CustomSection {
                section_name: group.name.trim().to_owned(),
                section_fields: group
                    .fields
                    .iter()
                    .filter(|field| !field.is_empty())
                    .map(extra_field)
                    .collect(),
            })
            .collect()
    }

    fn content(&self) -> ItemContent {
        match self.item_type {
            ItemType::Login => ItemContent::Login(ItemLogin {
                item_email: self.email.trim().to_owned(),
                item_username: self.username.trim().to_owned(),
                password: self.password.clone(),
                totp_uri: self.totp.trim().to_owned(),
                urls: lines(&self.urls),
                passkeys: Vec::new(),
                autofill_urls: Vec::new(),
            }),
            ItemType::Alias => ItemContent::Alias(ItemAlias {}),
            ItemType::Note => ItemContent::Note(ItemNote {}),
            ItemType::Card => ItemContent::CreditCard(ItemCreditCard {
                cardholder_name: self.card.holder.clone(),
                card_type: self.card.kind,
                number: self.card.number.replace([' ', '-'], ""),
                verification_number: self.card.verification.clone(),
                expiration_date: self.card.expiration.clone(),
                pin: self.card.pin.clone(),
            }),
            ItemType::Wifi => ItemContent::Wifi(ItemWifi {
                ssid: self.wifi.ssid.clone(),
                password: self.wifi.password.clone(),
                security: self.wifi.security,
                sections: self.sections(),
            }),
            ItemType::SshKey => ItemContent::SshKey(ItemSshKey {
                private_key: self.ssh.private.clone(),
                public_key: self.ssh.public.clone(),
                sections: self.sections(),
            }),
            ItemType::Identity => ItemContent::Identity(Box::new(ItemIdentity {
                extra_sections: self.sections(),
                ..self.identity.clone()
            })),
            ItemType::Custom => ItemContent::Custom(ItemCustom {
                sections: self.sections(),
            }),
        }
    }
}

fn extra_field(field: &CustomField) -> ExtraField {
    let content = match field.hidden {
        true => ExtraFieldContent::Hidden(ExtraHiddenField {
            content: field.value.clone(),
        }),
        false => ExtraFieldContent::Text(ExtraTextField {
            content: field.value.clone(),
        }),
    };

    ExtraField {
        field_name: field.name.trim().to_owned(),
        content: Some(content),
    }
}

fn extra_value_text(value: &ExtraValue<'_>) -> String {
    match value {
        ExtraValue::Text(text) | ExtraValue::Hidden(text) | ExtraValue::Totp(text) => {
            (*text).to_owned()
        }
        ExtraValue::Timestamp(at) => at.seconds.to_string(),
    }
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn custom_field(field: &ExtraField) -> CustomField {
    CustomField::new(
        field.name(),
        match field.value() {
            Some(value) => extra_value_text(&value),
            None => String::new(),
        },
        field.is_hidden(),
    )
}

impl fmt::Debug for Draft {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Draft")
            .field("share", &self.share)
            .field("id", &self.id)
            .field("item_type", &self.item_type)
            .finish_non_exhaustive()
    }
}
