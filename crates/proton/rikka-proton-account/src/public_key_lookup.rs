use rikka_proton_api::{ApiError, offload};

use crate::account::Account;
use crate::error::{Error, Result};
use crate::public_keys::PublicKeys;

const PUBLIC_KEYS_PATH: &str = "core/v4/keys/all";
const INVALID_INPUT: i32 = 33101;
const ADDRESS_MISSING: i32 = 33102;
const DOMAIN_EXTERNAL: i32 = 33103;
const INVALID_KEY_TRANSPARENCY: i32 = 33104;
const UNPUBLISHED_CODES: [i32; 4] = [
    INVALID_INPUT,
    ADDRESS_MISSING,
    DOMAIN_EXTERNAL,
    INVALID_KEY_TRANSPARENCY,
];

#[derive(Debug)]
pub enum PublishedAddress {
    Keys(Box<PublicKeys>),
    Unpublished(Box<ApiError>),
}

impl PublishedAddress {
    #[must_use]
    pub fn is_external_domain(&self) -> bool {
        matches!(self, PublishedAddress::Unpublished(error) if error.code.value() == DOMAIN_EXTERNAL)
    }

    pub fn into_public_keys(self) -> Result<PublicKeys> {
        match self {
            PublishedAddress::Keys(keys) => Ok(*keys),
            PublishedAddress::Unpublished(error) => {
                Err(Error::Api(rikka_proton_api::Error::Api(error)))
            }
        }
    }
}

#[must_use]
#[derive(Debug)]
pub struct PublicKeyLookup<'a> {
    account: &'a Account,
    email: &'a str,
    internal_only: bool,
}

impl<'a> PublicKeyLookup<'a> {
    pub(crate) fn new(account: &'a Account, email: &'a str) -> PublicKeyLookup<'a> {
        PublicKeyLookup {
            account,
            email,
            internal_only: false,
        }
    }

    pub fn internal_only(mut self) -> Self {
        self.internal_only = true;
        self
    }

    pub async fn send(self) -> Result<PublishedAddress> {
        let result = self
            .account
            .api()
            .get(PUBLIC_KEYS_PATH)
            .query(&[
                ("Email", self.email),
                ("InternalOnly", if self.internal_only { "1" } else { "0" }),
            ])
            .send::<PublicKeys>()
            .await;
        let keys = match result {
            Ok(keys) => keys,
            Err(rikka_proton_api::Error::Api(error))
                if UNPUBLISHED_CODES.contains(&error.code.value()) =>
            {
                return Ok(PublishedAddress::Unpublished(error));
            }
            Err(error) => return Err(error.into()),
        };

        let pgp = *self.account.pgp();
        let address_keys = keys.address().clone();
        let check = offload(move || address_keys.check_signed_key_list(&pgp)).await;
        if check.is_forged() {
            return Err(Error::UntrustedSignedKeyList {
                email: self.email.to_owned(),
                check: Box::new(check),
            });
        }
        if !check.is_trustworthy() {
            tracing::warn!(email = self.email, %check, "published keys do not match signed key list");
        }

        Ok(PublishedAddress::Keys(Box::new(keys)))
    }
}
