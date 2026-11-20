mod card;
mod codec;
mod custom;
mod extra;
mod file;
mod folder;
mod identity;
mod item;
mod login;
mod passkey;
mod platform;
mod ssh;
mod timestamp;
mod vault;
mod wifi;

pub use card::{CardType, Expiration, ItemCreditCard};
pub use custom::{CustomSection, ItemCustom};
pub use extra::{
    ExtraField, ExtraFieldContent, ExtraHiddenField, ExtraTextField, ExtraTimestampField,
    ExtraTotp, ExtraValue,
};
pub use file::FileMetadata;
pub use folder::Folder;
pub use identity::ItemIdentity;
pub use item::{Content, Item, ItemAlias, ItemContent, ItemKind, ItemNote, Metadata};
pub use login::{AutofillMode, AutofillUrl, ItemLogin};
pub use passkey::{Passkey, PasskeyCreationData};
pub use platform::{AllowedAndroidApp, AndroidSpecific, PlatformSpecific};
pub use ssh::ItemSshKey;
pub use timestamp::Timestamp;
pub use vault::{Vault, VaultColor, VaultDisplayPreferences, VaultIcon};
pub use wifi::{ItemWifi, WifiSecurity};

pub(crate) fn non_empty(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}
