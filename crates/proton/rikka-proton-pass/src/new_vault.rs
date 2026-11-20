use std::fmt;

use rikka_proton_account::AddressId;

use crate::content::{Vault, VaultColor, VaultDisplayPreferences, VaultIcon};

#[must_use]
#[derive(Clone)]
pub struct NewVault {
    pub(crate) vault: Vault,
    pub(crate) address: Option<AddressId>,
}

impl NewVault {
    pub fn new(name: impl Into<String>) -> NewVault {
        NewVault {
            vault: Vault {
                name: name.into(),
                description: String::new(),
                display: None,
            },
            address: None,
        }
    }

    pub fn from_content(vault: Vault) -> NewVault {
        NewVault {
            vault,
            address: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> NewVault {
        self.vault.description = description.into();
        self
    }

    pub fn icon(mut self, icon: VaultIcon) -> NewVault {
        self.display().icon = icon as i32;
        self
    }

    pub fn color(mut self, color: VaultColor) -> NewVault {
        self.display().color = color as i32;
        self
    }

    pub fn address(mut self, address: AddressId) -> NewVault {
        self.address = Some(address);
        self
    }

    fn display(&mut self) -> &mut VaultDisplayPreferences {
        self.vault.display.get_or_insert_default()
    }
}

impl fmt::Debug for NewVault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewVault")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}
