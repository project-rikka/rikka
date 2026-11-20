use std::ops::{BitOr, Sub};

use serde::{Deserialize, Serialize};

use crate::address::{Address, AddressType};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AddressKeyFlags(i32);

impl AddressKeyFlags {
    pub const NONE: AddressKeyFlags = AddressKeyFlags(0);
    pub const NOT_COMPROMISED: AddressKeyFlags = AddressKeyFlags(1);
    pub const NOT_OBSOLETE: AddressKeyFlags = AddressKeyFlags(2);
    pub const EMAIL_NO_ENCRYPT: AddressKeyFlags = AddressKeyFlags(4);
    pub const EMAIL_NO_SIGN: AddressKeyFlags = AddressKeyFlags(8);

    #[must_use]
    pub const fn from_bits(bits: i32) -> AddressKeyFlags {
        AddressKeyFlags(bits)
    }

    #[must_use]
    pub const fn bits(self) -> i32 {
        self.0
    }

    #[must_use]
    pub fn for_new_key(address: &Address) -> AddressKeyFlags {
        let external = address.kind() == AddressType::External;
        let mut flags = AddressKeyFlags::NOT_COMPROMISED | AddressKeyFlags::NOT_OBSOLETE;

        if external || !address.has_end_to_end_encryption() {
            flags = flags | AddressKeyFlags::EMAIL_NO_ENCRYPT;
        }
        if external || !address.expects_signed_mail() {
            flags = flags | AddressKeyFlags::EMAIL_NO_SIGN;
        }
        flags
    }

    #[must_use]
    pub fn for_reactivation(self) -> AddressKeyFlags {
        self - AddressKeyFlags::NOT_OBSOLETE
    }

    #[must_use]
    pub fn contains(self, other: AddressKeyFlags) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub fn can_verify(self) -> bool {
        self.contains(AddressKeyFlags::NOT_COMPROMISED)
    }

    #[must_use]
    pub fn can_encrypt(self) -> bool {
        self.contains(AddressKeyFlags::NOT_OBSOLETE)
    }

    #[must_use]
    pub fn encrypts_email(self) -> bool {
        !self.contains(AddressKeyFlags::EMAIL_NO_ENCRYPT)
    }

    #[must_use]
    pub fn expects_signed_email(self) -> bool {
        !self.contains(AddressKeyFlags::EMAIL_NO_SIGN)
    }
}

impl BitOr for AddressKeyFlags {
    type Output = AddressKeyFlags;

    fn bitor(self, other: AddressKeyFlags) -> AddressKeyFlags {
        AddressKeyFlags(self.0 | other.0)
    }
}

impl Sub for AddressKeyFlags {
    type Output = AddressKeyFlags;

    fn sub(self, other: AddressKeyFlags) -> AddressKeyFlags {
        AddressKeyFlags(self.0 & !other.0)
    }
}

impl From<i32> for AddressKeyFlags {
    fn from(bits: i32) -> AddressKeyFlags {
        AddressKeyFlags(bits)
    }
}

impl From<AddressKeyFlags> for i32 {
    fn from(flags: AddressKeyFlags) -> i32 {
        flags.0
    }
}
