use rikka_proton_api::offload;
use rikka_proton_pgp::KeyKind;

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::active_keys::ActiveKeys;
use crate::address::Address;
use crate::address_key::AddressKey;
use crate::error::{Error, Result};
use crate::identifier::{AddressId, KeyId};
use crate::key_flags::AddressKeyFlags;
use crate::key_setup::AddressKeyRequest;
use crate::new_key::{AddressKeyLock, NewAddressKey, key_version};

const MIGRATED_PATH: &str = "core/v4/keys/address";
const LEGACY_PATH: &str = "core/v4/keys";

#[must_use]
#[derive(Debug)]
pub struct AddressKeyCreation<'a> {
    account: Account,
    keys: &'a AccountKeys,
    address: AddressId,
    primary: bool,
    kind: KeyKind,
}

impl AddressKeyCreation<'_> {
    pub fn primary(mut self, primary: bool) -> Self {
        self.primary = primary;
        self
    }

    pub fn kind(mut self, kind: KeyKind) -> Self {
        self.kind = kind;
        self
    }

    pub async fn execute(self) -> Result<AddressKey> {
        let address = self.account.address(&self.address).await?;
        let user_key = self
            .keys
            .primary_user_key()
            .ok_or(Error::NoUserKey)?
            .clone();
        let passphrase = match uses_tokens(self.keys) {
            true => None,
            false if self.kind == KeyKind::PostQuantum => {
                return Err(Error::PostQuantumNeedsTokens);
            }
            false => Some(self.keys.passphrase().ok_or(Error::NoPassphrase)?.clone()),
        };
        let list = ActiveKeys::for_address(&address, self.keys);
        let primary = self.primary || !list.has_version(key_version(self.kind));
        let flags = AddressKeyFlags::for_new_key(&address);

        let pgp = *self.account.pgp();
        let email = address.email().to_owned();
        let address_id = address.id().clone();
        let kind = self.kind;
        let request = offload(move || {
            let lock = match &passphrase {
                Some(passphrase) => AddressKeyLock::Passphrase(passphrase),
                None => AddressKeyLock::Token(&user_key),
            };
            let generated = NewAddressKey::generate(&pgp, &email, kind, lock)?;
            let signed_key_list = list
                .with_key(KeyId::default(), generated.key.clone(), flags, primary)
                .sign(&pgp)?;
            Ok::<_, Error>(AddressKeyRequest::new(
                address_id,
                generated,
                primary,
                signed_key_list,
            ))
        })
        .await?;

        let path = match request.is_migrated() {
            true => MIGRATED_PATH,
            false => LEGACY_PATH,
        };
        Ok(self
            .account
            .api()
            .post(path)
            .json(&request)
            .send_field("Key")
            .await?)
    }

    pub(crate) fn new(
        account: Account,
        keys: &AccountKeys,
        address: AddressId,
    ) -> AddressKeyCreation<'_> {
        AddressKeyCreation {
            account,
            keys,
            address,
            primary: false,
            kind: KeyKind::default(),
        }
    }
}

pub(crate) fn uses_tokens(keys: &AccountKeys) -> bool {
    let mut address_keys = keys.addresses().iter().flat_map(Address::keys).peekable();

    address_keys.peek().is_none() || address_keys.any(AddressKey::is_migrated)
}
