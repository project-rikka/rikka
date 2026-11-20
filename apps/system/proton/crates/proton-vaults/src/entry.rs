use jiff::Timestamp;
use rikka_proton_pass::{ItemFlags, ItemId, ItemLogin, ItemState, PassItem, ShareId};

use crate::item_type::ItemType;
use crate::strength::Strength;

#[derive(Clone, Debug)]
pub struct Entry {
    pub share: ShareId,
    pub id: ItemId,
    pub item_type: ItemType,
    pub title: String,
    pub subtitle: String,
    pub website: Option<String>,
    pub pinned: bool,
    pub trashed: bool,
    pub has_code: bool,
    pub strength: Option<Strength>,
    pub flags: ItemFlags,
    pub revision: i64,
    pub created: Timestamp,
    pub modified: Timestamp,
    pub used: Timestamp,
}

impl Entry {
    #[must_use]
    pub fn from_item(item: &PassItem) -> Entry {
        let content = item.content();
        let item_type = ItemType::from_kind(item.kind());
        let response = item.response();
        let password = content
            .login()
            .map(|login| login.password.as_str())
            .filter(|password| !password.is_empty());

        Entry {
            share: item.share_id().clone(),
            id: item.id().clone(),
            item_type,
            title: item.name().to_owned(),
            subtitle: subtitle(item, item_type),
            website: content
                .login()
                .and_then(|login| login.urls.first().cloned()),
            pinned: item.is_pinned(),
            trashed: item.state() == ItemState::Trashed,
            has_code: !content.totp_uris().is_empty(),
            strength: password.map(Strength::estimate),
            flags: item.flags(),
            revision: item.revision(),
            created: response.create_time(),
            modified: response.modify_time(),
            used: response.last_use_time().unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn is_excluded(&self) -> bool {
        self.flags.skips_health_check()
    }

    pub(crate) fn folded(&self) -> String {
        let mut folded = self.title.to_lowercase();
        for text in std::iter::once(self.subtitle.as_str()).chain(self.website.as_deref()) {
            folded.push('\n');
            folded.push_str(&text.to_lowercase());
        }
        folded
    }
}

fn subtitle(item: &PassItem, item_type: ItemType) -> String {
    let content = item.content();

    match item_type {
        ItemType::Login => content
            .login()
            .and_then(ItemLogin::identifier)
            .unwrap_or_default()
            .to_owned(),
        ItemType::Alias => item.alias_email().unwrap_or_default().to_owned(),
        ItemType::Note => item
            .note()
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned(),
        ItemType::Card => content
            .credit_card()
            .map(|card| tail(&card.number))
            .unwrap_or_default(),
        ItemType::Wifi => content
            .wifi()
            .and_then(|wifi| wifi.network())
            .unwrap_or_default()
            .to_owned(),
        ItemType::Identity => content
            .identity()
            .map(|identity| identity.full_name.clone())
            .unwrap_or_default(),
        ItemType::SshKey | ItemType::Custom => String::new(),
    }
}

fn tail(number: &str) -> String {
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    match digits.len() > 4 {
        true => format!("•••• {}", &digits[digits.len() - 4..]),
        false => digits,
    }
}
