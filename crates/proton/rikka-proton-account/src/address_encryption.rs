use rikka_proton_api::offload;
use serde::Serialize;

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::active_keys::ActiveKeys;
use crate::address::AddressType;
use crate::error::Result;
use crate::identifier::AddressId;
use crate::key_flags::AddressKeyFlags;
use crate::signed_key_list::SignedKeyList;

#[must_use]
#[derive(Debug)]
pub struct AddressEncryption<'a> {
    account: Account,
    keys: &'a AccountKeys,
    address: AddressId,
    encrypt: bool,
    expect_signed: bool,
}

impl AddressEncryption<'_> {
    pub fn encrypt(mut self, encrypt: bool) -> Self {
        self.encrypt = encrypt;
        self
    }

    pub fn expect_signed(mut self, expect_signed: bool) -> Self {
        self.expect_signed = expect_signed;
        self
    }

    pub async fn execute(self) -> Result<()> {
        let address = self.account.address(&self.address).await?;
        let list = ActiveKeys::for_address(&address, self.keys);
        let pgp = *self.account.pgp();
        let (encrypt, expect_signed) = (self.encrypt, self.expect_signed);
        let signed_key_list = offload(move || {
            list.map_flags(|flags| e2ee_flags(flags, encrypt, expect_signed))
                .sign(&pgp)
        })
        .await?;

        self.account
            .api()
            .put(format!("core/v4/addresses/{}/encryption", self.address))
            .json(&EncryptionRequest {
                encrypt: u8::from(encrypt),
                sign: u8::from(expect_signed),
                signed_key_list,
            })
            .send_unit()
            .await?;

        Ok(())
    }
}

impl Account {
    pub fn address_encryption<'a>(
        &self,
        keys: &'a AccountKeys,
        address: &AddressId,
    ) -> AddressEncryption<'a> {
        AddressEncryption {
            account: self.clone(),
            keys,
            address: address.clone(),
            encrypt: true,
            expect_signed: true,
        }
    }

    pub async fn convert_to_internal_address(
        &self,
        keys: &AccountKeys,
        address: &AddressId,
    ) -> Result<()> {
        let current = self.address(address).await?;
        let list = ActiveKeys::for_address(&current, keys);
        let pgp = *self.pgp();
        let signed_key_list = offload(move || {
            list.map_flags(|flags| e2ee_flags(flags, true, true))
                .sign(&pgp)
        })
        .await?;

        self.api()
            .put(format!("core/v4/addresses/{address}/type"))
            .json(&TypeRequest {
                kind: AddressType::Custom,
                signed_key_list,
            })
            .send_unit()
            .await?;

        Ok(())
    }
}

fn e2ee_flags(flags: AddressKeyFlags, encrypt: bool, expect_signed: bool) -> AddressKeyFlags {
    let flags = match encrypt {
        true => flags - AddressKeyFlags::EMAIL_NO_ENCRYPT,
        false => flags | AddressKeyFlags::EMAIL_NO_ENCRYPT,
    };

    match expect_signed {
        true => flags - AddressKeyFlags::EMAIL_NO_SIGN,
        false => flags | AddressKeyFlags::EMAIL_NO_SIGN,
    }
}

#[derive(Debug, Serialize)]
struct EncryptionRequest {
    #[serde(rename = "Encrypt")]
    encrypt: u8,
    #[serde(rename = "Sign")]
    sign: u8,
    #[serde(rename = "SignedKeyList")]
    signed_key_list: SignedKeyList,
}

#[derive(Debug, Serialize)]
struct TypeRequest {
    #[serde(rename = "Type")]
    kind: AddressType,
    #[serde(rename = "SignedKeyList")]
    signed_key_list: SignedKeyList,
}
