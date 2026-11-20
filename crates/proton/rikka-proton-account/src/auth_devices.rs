use std::fmt;

use rikka_proton_api::offload;
use rikka_proton_pgp::{KeyRing, PrivateKey, Secret, SecretBytes};
use serde::{Deserialize, Serialize};

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::auth_device::{AuthDevice, AuthDeviceError, AuthDeviceId};
use crate::device_secret::{self, ConfirmationCodeVersion, DeviceSecret};
use crate::error::{Error, Result};
use crate::identifier::KeyId;
use crate::passphrase;
use crate::password_change::{RelockedKey, relock_keys};

const DEVICES_PATH: &str = "auth/v4/devices";
const MEMBERS_PATH: &str = "core/v4/members";
const CONFIRMATION_CODE_VERSION: ConfirmationCodeVersion = ConfirmationCodeVersion::HexDigest;
const MEMBER_PASSWORD_LEN: usize = 12;

impl Account {
    pub async fn auth_devices(&self) -> Result<Vec<AuthDevice>> {
        Ok(self
            .api()
            .get(DEVICES_PATH)
            .send_field_or_default("AuthDevices")
            .await?)
    }

    pub async fn auth_device(&self, id: &AuthDeviceId) -> Result<AuthDevice> {
        Ok(self
            .api()
            .get(format!("{DEVICES_PATH}/{id}"))
            .send_field("AuthDevice")
            .await?)
    }

    pub async fn create_auth_device(&self, name: &str) -> Result<CreatedDevice> {
        let profile = self.profile().await?;
        let secret = device_secret::generate_secret();

        let activation_key = match profile.user().keys().is_empty() {
            true => None,
            false => profile
                .addresses()
                .iter()
                .filter(|address| address.is_enabled())
                .find_map(|address| address.primary_key())
                .map(|key| key.to_locked_key())
                .transpose()?
                .map(|locked| locked.public_key()),
        };
        let activation_token = match activation_key {
            Some(key) => {
                let pgp = *self.pgp();
                let plaintext = secret.clone();
                Some(
                    offload(move || {
                        pgp.encrypt()
                            .to(key)
                            .as_utf8()
                            .armored_message(plaintext.expose())
                    })
                    .await?,
                )
            }
            None => None,
        };

        let request = CreateRequest {
            name,
            activation_token,
            confirmation_code_version: CONFIRMATION_CODE_VERSION.number(),
        };
        let response: CreatedAuthDevice = self
            .api()
            .post(DEVICES_PATH)
            .json(&request)
            .send_field("AuthDevice")
            .await?;
        let created = response;

        Ok(CreatedDevice {
            secret: DeviceSecret::new(
                created.device.id().clone(),
                created.token.expose(),
                secret.expose(),
            ),
            device: created.device,
        })
    }

    pub async fn device_passphrase(&self, secret: &DeviceSecret) -> Result<SecretBytes> {
        let request = AssociateRequest {
            token: secret.token().expose(),
        };
        let encrypted_secret: Secret = self
            .api()
            .post(format!("{DEVICES_PATH}/{}/associate", secret.device()))
            .json(&request)
            .send_field_at(&["AuthDevice", "EncryptedSecret"])
            .await
            .map_err(|error| match AuthDeviceError::from_api_error(&error) {
                Some(reason) => Error::AuthDevice {
                    device: secret.device().clone(),
                    reason,
                },
                None => error.into(),
            })?;

        secret.open(encrypted_secret.expose())
    }

    pub async fn unlock_with_device(&self, secret: &DeviceSecret) -> Result<AccountKeys> {
        let passphrase = self.device_passphrase(secret).await?;

        self.unlock_with_passphrase(passphrase.expose()).await
    }

    pub async fn activate_auth_device(
        &self,
        keys: &AccountKeys,
        secret: &DeviceSecret,
    ) -> Result<()> {
        let passphrase = keys.passphrase().ok_or(Error::NoPassphrase)?;

        self.activate_device(secret.device(), secret.seal(passphrase)?)
            .await
    }

    pub async fn approve_auth_device(
        &self,
        keys: &AccountKeys,
        device: &AuthDevice,
        confirmation_code: &str,
    ) -> Result<()> {
        let passphrase = keys.passphrase().ok_or(Error::NoPassphrase)?;
        let address = device
            .activation_address()
            .ok_or_else(|| Error::NoActivationToken(device.id().clone()))?;
        let ring = keys
            .address_keys(address)
            .ok_or_else(|| Error::NoPrimaryAddressKey(address.clone()))?;

        let secret = self
            .decrypt_confirmed_secret(device, ring, confirmation_code)
            .await?;

        self.activate_device(device.id(), device_secret::seal(&secret, passphrase)?)
            .await
    }

    pub async fn reject_auth_device(&self, id: &AuthDeviceId) -> Result<()> {
        self.api()
            .put(format!("{DEVICES_PATH}/{id}/reject"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_auth_device(&self, id: &AuthDeviceId) -> Result<()> {
        self.api()
            .delete(format!("{DEVICES_PATH}/{id}"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_other_auth_devices(&self) -> Result<()> {
        self.api().delete(DEVICES_PATH).send_unit().await?;

        Ok(())
    }

    pub async fn request_admin_approval(&self, id: &AuthDeviceId) -> Result<()> {
        self.api()
            .put(format!("{DEVICES_PATH}/{id}/admin"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn revoke_session_and_device(&self) -> Result<()> {
        self.api()
            .delete("auth/v4")
            .query(&[("AuthDevice", 1)])
            .send_unit()
            .await?;
        self.api().clear_session()?;

        Ok(())
    }

    pub async fn pending_member_devices(&self) -> Result<Vec<AuthDevice>> {
        Ok(self
            .api()
            .get(format!("{MEMBERS_PATH}/devices/pending"))
            .send_field_or_default("MemberAuthDevices")
            .await?)
    }

    pub async fn member_auth_device(&self, id: &AuthDeviceId) -> Result<AuthDevice> {
        Ok(self
            .api()
            .get(format!("{MEMBERS_PATH}/devices/{id}"))
            .send_field("MemberAuthDevice")
            .await?)
    }

    pub async fn approve_member_device(
        &self,
        device: &AuthDevice,
        confirmation_code: &str,
        member: &MemberKeys,
    ) -> Result<()> {
        let member_id = device.member_id().ok_or(Error::Incomplete("MemberID"))?;
        let secret = self
            .decrypt_confirmed_secret(device, &member.address_keys, confirmation_code)
            .await?;

        let password = Secret::new(rikka_proton_api::base64::encode(
            rikka_proton_pgp::random_bytes::<MEMBER_PASSWORD_LEN>(),
        ));
        let salted = passphrase::derive_with_new_salt(password).await?;
        let user_keys = relock_keys(member.user_keys.clone(), &salted.passphrase).await?;

        let request = MemberResetRequest {
            device: device.id(),
            encrypted_secret: device_secret::seal(&secret, &salted.passphrase)?,
            user_keys,
        };
        self.api()
            .post(format!("{MEMBERS_PATH}/{member_id}/devices/reset"))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn reject_member_device(&self, device: &AuthDevice) -> Result<()> {
        let member_id = device.member_id().ok_or(Error::Incomplete("MemberID"))?;

        self.api()
            .put(format!(
                "{MEMBERS_PATH}/{member_id}/devices/{}/reject",
                device.id()
            ))
            .send_unit()
            .await?;

        Ok(())
    }

    async fn activate_device(&self, id: &AuthDeviceId, encrypted_secret: String) -> Result<()> {
        self.api()
            .post(format!("{DEVICES_PATH}/{id}"))
            .json(&ActivateRequest { encrypted_secret })
            .send_unit()
            .await?;

        Ok(())
    }

    async fn decrypt_confirmed_secret(
        &self,
        device: &AuthDevice,
        ring: &KeyRing,
        confirmation_code: &str,
    ) -> Result<Secret> {
        let token = device
            .activation_token()
            .ok_or_else(|| Error::NoActivationToken(device.id().clone()))?
            .to_owned();
        let version =
            ConfirmationCodeVersion::for_device(device.id(), device.confirmation_code_version())?;

        let pgp = *self.pgp();
        let ring = ring.clone();
        let decrypted = offload(move || pgp.decrypt().with_ring(&ring).message(token)).await?;
        let secret = Secret::new(
            String::from_utf8(decrypted.into_data()).map_err(|_| Error::MalformedDeviceSecret)?,
        );

        let expected = device_secret::confirmation_code(&secret, version);
        if !device_secret::codes_match(&expected, confirmation_code) {
            return Err(Error::WrongConfirmationCode(device.id().clone()));
        }

        Ok(secret)
    }
}

#[derive(Debug, Clone)]
pub struct CreatedDevice {
    device: AuthDevice,
    secret: DeviceSecret,
}

impl CreatedDevice {
    #[must_use]
    pub fn device(&self) -> &AuthDevice {
        &self.device
    }

    #[must_use]
    pub fn secret(&self) -> &DeviceSecret {
        &self.secret
    }

    #[must_use]
    pub fn confirmation_code(&self) -> String {
        self.secret.confirmation_code(CONFIRMATION_CODE_VERSION)
    }

    #[must_use]
    pub fn into_parts(self) -> (AuthDevice, DeviceSecret) {
        (self.device, self.secret)
    }
}

#[derive(Clone, Default)]
pub struct MemberKeys {
    user_keys: Vec<(KeyId, PrivateKey)>,
    address_keys: KeyRing,
}

impl MemberKeys {
    #[must_use]
    pub fn new() -> MemberKeys {
        MemberKeys::default()
    }

    #[must_use]
    pub fn user_key(mut self, id: KeyId, key: PrivateKey) -> MemberKeys {
        self.user_keys.push((id, key));
        self
    }

    #[must_use]
    pub fn address_key(mut self, key: PrivateKey) -> MemberKeys {
        self.address_keys = self.address_keys.with_private(key);
        self
    }
}

impl fmt::Debug for MemberKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemberKeys")
            .field("user_keys", &self.user_keys.len())
            .field("address_keys", &self.address_keys.len())
            .finish()
    }
}

#[derive(Debug, Serialize)]
struct CreateRequest<'a> {
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "ActivationToken", skip_serializing_if = "Option::is_none")]
    activation_token: Option<String>,
    #[serde(rename = "ConfirmationCodeVersion")]
    confirmation_code_version: u8,
}

#[derive(Debug, Serialize)]
struct AssociateRequest<'a> {
    #[serde(rename = "DeviceToken")]
    token: &'a str,
}

#[derive(Debug, Serialize)]
struct ActivateRequest {
    #[serde(rename = "EncryptedSecret")]
    encrypted_secret: String,
}

#[derive(Debug, Serialize)]
struct MemberResetRequest<'a> {
    #[serde(rename = "AuthDeviceID")]
    device: &'a AuthDeviceId,
    #[serde(rename = "EncryptedSecret")]
    encrypted_secret: String,
    #[serde(rename = "UserKeys")]
    user_keys: Vec<RelockedKey>,
}

#[derive(Debug, Deserialize)]
struct CreatedAuthDevice {
    #[serde(flatten)]
    device: AuthDevice,
    #[serde(rename = "DeviceToken")]
    token: Secret,
}
