use rikka_proton_api::offload;
use serde::Serialize;

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::active_keys::ActiveKeys;
use crate::address::Address;
use crate::address_key_creation::AddressKeyCreation;
use crate::error::{Error, Result};
use crate::identifier::{AddressId, KeyId};
use crate::key_flags::AddressKeyFlags;
use crate::signed_key_list::SignedKeyList;

impl Account {
    pub fn create_address_key<'a>(
        &self,
        keys: &'a AccountKeys,
        address: &AddressId,
    ) -> AddressKeyCreation<'a> {
        AddressKeyCreation::new(self.clone(), keys, address.clone())
    }

    pub async fn set_primary_address_key(&self, keys: &AccountKeys, key: &KeyId) -> Result<()> {
        let address = self.address_holding_key(keys, key).await?;
        let list = ActiveKeys::for_address(&address, keys);
        let id = key.clone();
        let pgp = *self.pgp();
        let signed_key_list = offload(move || list.with_primary(&id)?.sign(&pgp)).await?;

        self.send_primary(key, true, signed_key_list).await
    }

    pub async fn unset_v6_primary_address_key(
        &self,
        keys: &AccountKeys,
        key: &KeyId,
    ) -> Result<()> {
        let address = self.address_holding_key(keys, key).await?;
        let list = ActiveKeys::for_address(&address, keys);
        let id = key.clone();
        let pgp = *self.pgp();
        let signed_key_list = offload(move || list.without_v6_primary(&id)?.sign(&pgp)).await?;

        self.send_primary(key, false, signed_key_list).await
    }

    async fn send_primary(
        &self,
        key: &KeyId,
        primary: bool,
        signed_key_list: SignedKeyList,
    ) -> Result<()> {
        self.api()
            .put(format!("core/v4/keys/{key}/primary"))
            .json(&PrimaryRequest {
                signed_key_list,
                primary: u8::from(primary),
            })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_address_key_flags(
        &self,
        keys: &AccountKeys,
        key: &KeyId,
        flags: AddressKeyFlags,
    ) -> Result<()> {
        let address = self.address_holding_key(keys, key).await?;
        let list = ActiveKeys::for_address(&address, keys);
        let id = key.clone();
        let pgp = *self.pgp();
        let signed_key_list = offload(move || list.with_flags(&id, flags)?.sign(&pgp)).await?;

        self.api()
            .put(format!("core/v4/keys/{key}/flags"))
            .json(&FlagsRequest {
                flags,
                signed_key_list,
            })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_address_key(&self, keys: &AccountKeys, key: &KeyId) -> Result<()> {
        let address = self.address_holding_key(keys, key).await?;
        let list = ActiveKeys::for_address(&address, keys);
        if list.is_primary(key) {
            return Err(Error::PrimaryKeyDeletion(key.clone()));
        }
        let id = key.clone();
        let pgp = *self.pgp();
        let signed_key_list = offload(move || list.without_key(&id).sign_if_any(&pgp)).await?;

        self.api()
            .post(format!("core/v4/keys/address/{key}/delete"))
            .json(&SignedKeyListRequest { signed_key_list })
            .send_unit()
            .await?;

        Ok(())
    }

    pub(crate) async fn address_holding_key(
        &self,
        keys: &AccountKeys,
        key: &KeyId,
    ) -> Result<Address> {
        let address = keys
            .addresses()
            .iter()
            .find(|address| address.key(key).is_some())
            .ok_or_else(|| Error::NoSuchKey(key.clone()))?;

        self.address(address.id()).await
    }
}

#[derive(Debug, Serialize)]
struct SignedKeyListRequest {
    #[serde(rename = "SignedKeyList")]
    signed_key_list: Option<SignedKeyList>,
}

#[derive(Debug, Serialize)]
struct PrimaryRequest {
    #[serde(rename = "SignedKeyList")]
    signed_key_list: SignedKeyList,
    #[serde(rename = "Primary")]
    primary: u8,
}

#[derive(Debug, Serialize)]
struct FlagsRequest {
    #[serde(rename = "Flags")]
    flags: AddressKeyFlags,
    #[serde(rename = "SignedKeyList")]
    signed_key_list: SignedKeyList,
}
