use crate::content::card::ItemCreditCard;
use crate::content::custom::{CustomSection, ItemCustom};
use crate::content::extra::ExtraField;
use crate::content::identity::ItemIdentity;
use crate::content::login::ItemLogin;
use crate::content::non_empty;
use crate::content::platform::PlatformSpecific;
use crate::content::ssh::ItemSshKey;
use crate::content::wifi::ItemWifi;

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ItemNote {}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ItemAlias {}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct Metadata {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub note: String,
    #[prost(string, tag = "3")]
    pub item_uuid: String,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub custom_icon: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct Content {
    #[prost(oneof = "ItemContent", tags = "2, 3, 4, 5, 6, 7, 8, 9")]
    pub content: Option<ItemContent>,
}

#[derive(Clone, PartialEq, Eq, prost::Oneof)]
pub enum ItemContent {
    #[prost(message, tag = "2")]
    Note(ItemNote),
    #[prost(message, tag = "3")]
    Login(ItemLogin),
    #[prost(message, tag = "4")]
    Alias(ItemAlias),
    #[prost(message, tag = "5")]
    CreditCard(ItemCreditCard),
    #[prost(message, tag = "6", boxed)]
    Identity(Box<ItemIdentity>),
    #[prost(message, tag = "7")]
    SshKey(ItemSshKey),
    #[prost(message, tag = "8")]
    Wifi(ItemWifi),
    #[prost(message, tag = "9")]
    Custom(ItemCustom),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Note,
    Login,
    Alias,
    CreditCard,
    Identity,
    SshKey,
    Wifi,
    Custom,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct Item {
    #[prost(message, optional, tag = "1")]
    pub metadata: Option<Metadata>,
    #[prost(message, optional, tag = "2")]
    pub content: Option<Content>,
    #[prost(message, optional, tag = "3")]
    pub platform_specific: Option<PlatformSpecific>,
    #[prost(message, repeated, tag = "4")]
    pub extra_fields: Vec<ExtraField>,
}

impl ItemContent {
    pub fn kind(&self) -> ItemKind {
        match self {
            ItemContent::Note(_) => ItemKind::Note,
            ItemContent::Login(_) => ItemKind::Login,
            ItemContent::Alias(_) => ItemKind::Alias,
            ItemContent::CreditCard(_) => ItemKind::CreditCard,
            ItemContent::Identity(_) => ItemKind::Identity,
            ItemContent::SshKey(_) => ItemKind::SshKey,
            ItemContent::Wifi(_) => ItemKind::Wifi,
            ItemContent::Custom(_) => ItemKind::Custom,
        }
    }

    pub fn sections(&self) -> &[CustomSection] {
        match self {
            ItemContent::Custom(custom) => &custom.sections,
            ItemContent::SshKey(key) => &key.sections,
            ItemContent::Wifi(wifi) => &wifi.sections,
            ItemContent::Identity(identity) => &identity.extra_sections,
            _ => &[],
        }
    }
}

impl Item {
    pub fn new(metadata: Metadata, content: ItemContent) -> Item {
        Item {
            metadata: Some(metadata),
            content: Some(Content {
                content: Some(content),
            }),
            platform_specific: None,
            extra_fields: Vec::new(),
        }
    }

    pub fn body(&self) -> Option<&ItemContent> {
        self.content.as_ref()?.content.as_ref()
    }

    pub fn kind(&self) -> Option<ItemKind> {
        Some(self.body()?.kind())
    }

    pub fn name(&self) -> &str {
        self.metadata.as_ref().map_or("", |data| data.name.as_str())
    }

    pub fn note(&self) -> Option<&str> {
        non_empty(self.metadata.as_ref()?.note.as_str())
    }

    pub fn uuid(&self) -> Option<&str> {
        non_empty(self.metadata.as_ref()?.item_uuid.as_str())
    }

    pub fn custom_icon(&self) -> Option<&[u8]> {
        self.metadata.as_ref()?.custom_icon.as_deref()
    }

    pub fn login(&self) -> Option<&ItemLogin> {
        match self.body()? {
            ItemContent::Login(login) => Some(login),
            _ => None,
        }
    }

    pub fn credit_card(&self) -> Option<&ItemCreditCard> {
        match self.body()? {
            ItemContent::CreditCard(card) => Some(card),
            _ => None,
        }
    }

    pub fn identity(&self) -> Option<&ItemIdentity> {
        match self.body()? {
            ItemContent::Identity(identity) => Some(identity),
            _ => None,
        }
    }

    pub fn ssh_key(&self) -> Option<&ItemSshKey> {
        match self.body()? {
            ItemContent::SshKey(key) => Some(key),
            _ => None,
        }
    }

    pub fn wifi(&self) -> Option<&ItemWifi> {
        match self.body()? {
            ItemContent::Wifi(wifi) => Some(wifi),
            _ => None,
        }
    }

    pub fn custom(&self) -> Option<&ItemCustom> {
        match self.body()? {
            ItemContent::Custom(custom) => Some(custom),
            _ => None,
        }
    }

    pub fn sections(&self) -> &[CustomSection] {
        self.body().map_or(&[], ItemContent::sections)
    }

    pub fn fields(&self) -> impl Iterator<Item = &ExtraField> {
        let sections = self
            .sections()
            .iter()
            .flat_map(|section| &section.section_fields);
        let details = self.identity().into_iter().flat_map(ItemIdentity::details);
        self.extra_fields.iter().chain(details).chain(sections)
    }

    pub fn field(&self, name: &str) -> Option<&ExtraField> {
        self.fields().find(|field| field.field_name == name)
    }

    pub fn totp_uris(&self) -> Vec<&str> {
        let login = self.login().and_then(ItemLogin::totp);
        login
            .into_iter()
            .chain(self.fields().filter_map(ExtraField::totp_uri))
            .collect()
    }
}
