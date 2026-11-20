use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::address::Address;
use crate::address_key::AddressKey;
use crate::identifier::AddressId;
use crate::user::User;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(rename = "User")]
    user: User,
    #[serde(rename = "Addresses")]
    addresses: Vec<Address>,
}

impl Profile {
    #[must_use]
    pub fn new(user: User, mut addresses: Vec<Address>) -> Profile {
        addresses.sort_by_key(Address::order);
        Profile { user, addresses }
    }

    #[must_use]
    pub fn user(&self) -> &User {
        &self.user
    }

    #[must_use]
    pub fn addresses(&self) -> &[Address] {
        &self.addresses
    }

    #[must_use]
    pub fn has_same_keys(&self, other: &Profile) -> bool {
        self.user.keys() == other.user.keys() && self.address_keys() == other.address_keys()
    }

    pub(crate) fn into_parts(self) -> (User, Vec<Address>) {
        (self.user, self.addresses)
    }

    fn address_keys(&self) -> BTreeMap<&AddressId, &[AddressKey]> {
        self.addresses
            .iter()
            .map(|address| (address.id(), address.keys()))
            .collect()
    }
}
