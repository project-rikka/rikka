use rikka_proton_api::offload;
use rikka_proton_pgp::{KeyKind, Pgp, PrivateKey, Secret, SecretBytes};
use serde::Serialize;

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::address::Address;
use crate::device_secret::DeviceSecret;
use crate::error::{Error, Result};
use crate::identifier::AddressId;
use crate::key_flags::AddressKeyFlags;
use crate::key_list::SignedKeyListItem;
use crate::new_key::{AddressKeyLock, NewAddressKey, NewUserKey};
use crate::passphrase;
use crate::signed_key_list::SignedKeyList;
use crate::unprivatization::VerifiedUnprivatization;
use crate::verifier::{self, Verifier};

const SETUP_PATH: &str = "core/v4/keys/setup";

#[must_use]
#[derive(Debug)]
pub struct KeySetup {
    account: Account,
    password: Secret,
    device_secret: Option<DeviceSecret>,
    unprivatization: Option<VerifiedUnprivatization>,
}

impl KeySetup {
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Secret::new(password);
        self
    }

    pub fn device_secret(mut self, secret: &DeviceSecret) -> Self {
        self.device_secret = Some(secret.clone());
        self
    }

    pub fn unprivatization(mut self, unprivatization: &VerifiedUnprivatization) -> Self {
        self.unprivatization = Some(unprivatization.clone());
        self
    }

    pub async fn execute(self) -> Result<AccountKeys> {
        let (user, addresses) =
            futures_util::try_join!(self.account.user(), self.account.addresses())?;

        if !user.keys().is_empty() {
            return Err(Error::KeysAlreadySetUp);
        }

        let salted = passphrase::derive_with_new_salt(self.password.clone()).await?;
        let auth = verifier::generate(&self.account, &self.password).await?;
        let encrypted_secret = self
            .device_secret
            .as_ref()
            .map(|secret| secret.seal(&salted.passphrase))
            .transpose()?;

        let pgp = *self.account.pgp();
        let passphrase = salted.passphrase.clone();
        let keyless: Vec<Address> = addresses
            .into_iter()
            .filter(|address| address.keys().is_empty())
            .collect();
        let unprivatization = self.unprivatization;
        let generated =
            offload(move || generate_keys(&pgp, &passphrase, &keyless, unprivatization.as_ref()))
                .await?;

        let (organization_token, organization_user_key) = match generated.organization {
            Some(organization) => (
                Some(organization.activation_token),
                Some(organization.user_key),
            ),
            None => (None, None),
        };
        let request = SetupRequest {
            primary_key: generated.user_key,
            key_salt: salted.salt,
            address_keys: generated.address_keys,
            auth,
            encrypted_secret,
            organization_token,
            organization_user_key,
        };

        self.account
            .api()
            .post(SETUP_PATH)
            .json(&request)
            .send_unit()
            .await?;

        self.account
            .unlock_with_passphrase(salted.passphrase.expose())
            .await
    }

    pub(crate) fn new(account: Account) -> KeySetup {
        KeySetup {
            account,
            password: Secret::default(),
            device_secret: None,
            unprivatization: None,
        }
    }
}

struct GeneratedKeys {
    user_key: String,
    address_keys: Vec<AddressKeyRequest>,
    organization: Option<OrganizationKeys>,
}

struct OrganizationKeys {
    activation_token: String,
    user_key: String,
}

fn generate_keys(
    pgp: &Pgp,
    passphrase: &SecretBytes,
    keyless: &[Address],
    unprivatization: Option<&VerifiedUnprivatization>,
) -> Result<GeneratedKeys> {
    let user_key = NewUserKey::generate(pgp, KeyKind::Curve25519, passphrase)?;

    let generated: Vec<(&Address, NewAddressKey)> = keyless
        .iter()
        .map(|address| {
            let key = NewAddressKey::generate(
                pgp,
                address.email(),
                KeyKind::Curve25519,
                AddressKeyLock::Token(&user_key.key),
            )?;
            Ok((address, key))
        })
        .collect::<Result<_>>()?;

    let organization = unprivatization
        .map(|unprivatization| {
            let signer = invitation_key(&generated, unprivatization)?;
            let (token, activation_token) = unprivatization.encrypt_token(pgp, signer)?;
            let user_key = user_key.key.lock(token.expose())?.export_armored()?;
            Ok::<_, Error>(OrganizationKeys {
                activation_token,
                user_key,
            })
        })
        .transpose()?;

    let address_keys = generated
        .into_iter()
        .map(|(address, key)| {
            let item = SignedKeyListItem::new(
                &key.key.public_key(),
                true,
                AddressKeyFlags::for_new_key(address),
            );
            let signed_key_list = SignedKeyList::sign(pgp, &[item], &key.key)?;
            Ok(AddressKeyRequest::new(
                address.id().clone(),
                key,
                true,
                signed_key_list,
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(GeneratedKeys {
        user_key: user_key.armored,
        address_keys,
        organization,
    })
}

fn invitation_key<'a>(
    generated: &'a [(&Address, NewAddressKey)],
    unprivatization: &VerifiedUnprivatization,
) -> Result<&'a PrivateKey> {
    let (_, key) = match unprivatization.invitation_address() {
        Some(email) => generated
            .iter()
            .find(|(address, _)| address.email().eq_ignore_ascii_case(email))
            .ok_or_else(|| Error::NoInvitationAddress(email.to_owned()))?,
        None => generated.first().ok_or(Error::NoPrimaryKey)?,
    };

    Ok(&key.key)
}

#[derive(Debug, Serialize)]
pub(crate) struct AddressKeyRequest {
    #[serde(rename = "AddressID")]
    address_id: AddressId,
    #[serde(rename = "PrivateKey")]
    private_key: String,
    #[serde(rename = "Primary")]
    primary: u8,
    #[serde(rename = "Token", skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(rename = "Signature", skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(rename = "SignedKeyList")]
    signed_key_list: SignedKeyList,
}

impl AddressKeyRequest {
    pub(crate) fn new(
        address_id: AddressId,
        key: NewAddressKey,
        primary: bool,
        signed_key_list: SignedKeyList,
    ) -> AddressKeyRequest {
        let (token, signature) = match key.token {
            Some(token) => (Some(token.encrypted), Some(token.signature)),
            None => (None, None),
        };

        AddressKeyRequest {
            address_id,
            private_key: key.armored,
            primary: u8::from(primary),
            token,
            signature,
            signed_key_list,
        }
    }

    pub(crate) fn is_migrated(&self) -> bool {
        self.token.is_some()
    }
}

#[derive(Debug, Serialize)]
struct SetupRequest {
    #[serde(rename = "PrimaryKey")]
    primary_key: String,
    #[serde(rename = "KeySalt")]
    key_salt: String,
    #[serde(rename = "AddressKeys")]
    address_keys: Vec<AddressKeyRequest>,
    #[serde(rename = "Auth")]
    auth: Verifier,
    #[serde(rename = "EncryptedSecret", skip_serializing_if = "Option::is_none")]
    encrypted_secret: Option<String>,
    #[serde(rename = "OrgActivationToken", skip_serializing_if = "Option::is_none")]
    organization_token: Option<String>,
    #[serde(rename = "OrgPrimaryUserKey", skip_serializing_if = "Option::is_none")]
    organization_user_key: Option<String>,
}
