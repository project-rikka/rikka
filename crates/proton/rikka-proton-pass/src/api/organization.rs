use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

wire_enum! {
    pub enum ShareMode {
        Unrestricted = 0,
        InsideOrganization = 1,
    }
}

wire_enum! {
    pub enum ItemShareMode {
        Disabled = 0,
        Allowed = 1,
    }
}

wire_enum! {
    pub enum SecureLinkMode {
        Disabled = 0,
        Allowed = 1,
    }
}

wire_enum! {
    pub enum ExportMode {
        Unrestricted = 0,
        AdminsOnly = 1,
    }
}

wire_enum! {
    pub enum VaultCreateMode {
        Allowed = 0,
        AdminsOnly = 1,
        AdminsAndPersonalVault = 2,
    }
}

wire_enum! {
    pub enum AliasCreateMode {
        Allowed = 0,
        Nobody = 1,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PasswordPolicy {
    #[serde(rename = "RandomPasswordAllowed", default)]
    random_allowed: bool,
    #[serde(rename = "RandomPasswordMinLength", default)]
    random_min_length: Option<u32>,
    #[serde(rename = "RandomPasswordMaxLength", default)]
    random_max_length: Option<u32>,
    #[serde(rename = "RandomPasswordMustIncludeNumbers", default)]
    random_numbers: Option<bool>,
    #[serde(rename = "RandomPasswordMustIncludeSymbols", default)]
    random_symbols: Option<bool>,
    #[serde(rename = "RandomPasswordMustIncludeUppercase", default)]
    random_uppercase: Option<bool>,
    #[serde(rename = "MemorablePasswordAllowed", default)]
    memorable_allowed: bool,
    #[serde(rename = "MemorablePasswordMinWords", default)]
    memorable_min_words: Option<u32>,
    #[serde(rename = "MemorablePasswordMaxWords", default)]
    memorable_max_words: Option<u32>,
    #[serde(rename = "MemorablePasswordMustCapitalize", default)]
    memorable_capitalize: Option<bool>,
    #[serde(rename = "MemorablePasswordMustIncludeNumbers", default)]
    memorable_numbers: Option<bool>,
}

impl PasswordPolicy {
    #[must_use]
    pub fn new(random_allowed: bool, memorable_allowed: bool) -> PasswordPolicy {
        PasswordPolicy {
            random_allowed,
            memorable_allowed,
            ..PasswordPolicy::default()
        }
    }

    #[must_use]
    pub fn with_random_length(mut self, min: Option<u32>, max: Option<u32>) -> PasswordPolicy {
        self.random_min_length = min;
        self.random_max_length = max;
        self
    }

    #[must_use]
    pub fn with_random_numbers(mut self, required: Option<bool>) -> PasswordPolicy {
        self.random_numbers = required;
        self
    }

    #[must_use]
    pub fn with_random_symbols(mut self, required: Option<bool>) -> PasswordPolicy {
        self.random_symbols = required;
        self
    }

    #[must_use]
    pub fn with_random_uppercase(mut self, required: Option<bool>) -> PasswordPolicy {
        self.random_uppercase = required;
        self
    }

    #[must_use]
    pub fn with_memorable_words(mut self, min: Option<u32>, max: Option<u32>) -> PasswordPolicy {
        self.memorable_min_words = min;
        self.memorable_max_words = max;
        self
    }

    #[must_use]
    pub fn with_memorable_capitalize(mut self, required: Option<bool>) -> PasswordPolicy {
        self.memorable_capitalize = required;
        self
    }

    #[must_use]
    pub fn with_memorable_numbers(mut self, required: Option<bool>) -> PasswordPolicy {
        self.memorable_numbers = required;
        self
    }

    #[must_use]
    pub fn allows_random(&self) -> bool {
        self.random_allowed
    }

    #[must_use]
    pub fn random_min_length(&self) -> Option<u32> {
        self.random_min_length
    }

    #[must_use]
    pub fn random_max_length(&self) -> Option<u32> {
        self.random_max_length
    }

    #[must_use]
    pub fn random_numbers(&self) -> Option<bool> {
        self.random_numbers
    }

    #[must_use]
    pub fn random_symbols(&self) -> Option<bool> {
        self.random_symbols
    }

    #[must_use]
    pub fn random_uppercase(&self) -> Option<bool> {
        self.random_uppercase
    }

    #[must_use]
    pub fn allows_memorable(&self) -> bool {
        self.memorable_allowed
    }

    #[must_use]
    pub fn memorable_min_words(&self) -> Option<u32> {
        self.memorable_min_words
    }

    #[must_use]
    pub fn memorable_max_words(&self) -> Option<u32> {
        self.memorable_max_words
    }

    #[must_use]
    pub fn memorable_capitalize(&self) -> Option<bool> {
        self.memorable_capitalize
    }

    #[must_use]
    pub fn memorable_numbers(&self) -> Option<bool> {
        self.memorable_numbers
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationSettings {
    #[serde(rename = "ShareMode", default = "unrestricted_sharing")]
    share_mode: ShareMode,
    #[serde(rename = "ShareAcceptMode", default)]
    share_accept_mode: Option<ShareMode>,
    #[serde(rename = "ItemShareMode", default)]
    item_share_mode: Option<ItemShareMode>,
    #[serde(rename = "PublicLinkMode", default)]
    secure_link_mode: Option<SecureLinkMode>,
    #[serde(rename = "ForceLockSeconds", default)]
    force_lock_seconds: u64,
    #[serde(rename = "ExportMode", default)]
    export_mode: Option<ExportMode>,
    #[serde(rename = "PasswordPolicy", default)]
    password_policy: Option<PasswordPolicy>,
    #[serde(rename = "VaultCreateMode", default)]
    vault_create_mode: Option<VaultCreateMode>,
    #[serde(rename = "AliasCreateMode", default)]
    alias_create_mode: Option<AliasCreateMode>,
}

fn unrestricted_sharing() -> ShareMode {
    ShareMode::Unrestricted
}

impl OrganizationSettings {
    #[must_use]
    pub fn share_mode(&self) -> ShareMode {
        self.share_mode
    }

    #[must_use]
    pub fn share_accept_mode(&self) -> ShareMode {
        self.share_accept_mode.unwrap_or(ShareMode::Unrestricted)
    }

    #[must_use]
    pub fn item_share_mode(&self) -> ItemShareMode {
        self.item_share_mode.unwrap_or(ItemShareMode::Allowed)
    }

    #[must_use]
    pub fn secure_link_mode(&self) -> SecureLinkMode {
        self.secure_link_mode.unwrap_or(SecureLinkMode::Allowed)
    }

    #[must_use]
    pub fn force_lock_seconds(&self) -> Option<u64> {
        (self.force_lock_seconds > 0).then_some(self.force_lock_seconds)
    }

    #[must_use]
    pub fn export_mode(&self) -> ExportMode {
        self.export_mode.unwrap_or(ExportMode::Unrestricted)
    }

    #[must_use]
    pub fn password_policy(&self) -> Option<&PasswordPolicy> {
        self.password_policy.as_ref()
    }

    #[must_use]
    pub fn vault_create_mode(&self) -> VaultCreateMode {
        self.vault_create_mode.unwrap_or(VaultCreateMode::Allowed)
    }

    #[must_use]
    pub fn alias_create_mode(&self) -> AliasCreateMode {
        self.alias_create_mode.unwrap_or(AliasCreateMode::Allowed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    #[serde(rename = "CanUpdate", default)]
    can_update: bool,
    #[serde(rename = "Settings", default)]
    settings: Option<OrganizationSettings>,
}

impl Organization {
    #[must_use]
    pub fn can_update(&self) -> bool {
        self.can_update
    }

    #[must_use]
    pub fn settings(&self) -> Option<&OrganizationSettings> {
        self.settings.as_ref()
    }
}
