use rikka_proton_pass::ItemKind;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ItemType {
    #[default]
    Login,
    Alias,
    Note,
    Card,
    Identity,
    SshKey,
    Wifi,
    Custom,
}

impl ItemType {
    pub const ALL: [ItemType; 8] = [
        ItemType::Login,
        ItemType::Alias,
        ItemType::Note,
        ItemType::Card,
        ItemType::Identity,
        ItemType::SshKey,
        ItemType::Wifi,
        ItemType::Custom,
    ];

    #[must_use]
    pub fn from_kind(kind: Option<ItemKind>) -> ItemType {
        match kind {
            Some(ItemKind::Login) | None => ItemType::Login,
            Some(ItemKind::Alias) => ItemType::Alias,
            Some(ItemKind::Note) => ItemType::Note,
            Some(ItemKind::CreditCard) => ItemType::Card,
            Some(ItemKind::Identity) => ItemType::Identity,
            Some(ItemKind::SshKey) => ItemType::SshKey,
            Some(ItemKind::Wifi) => ItemType::Wifi,
            Some(ItemKind::Custom) => ItemType::Custom,
        }
    }
}
