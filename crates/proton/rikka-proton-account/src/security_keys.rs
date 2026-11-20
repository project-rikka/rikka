use base64::prelude::{BASE64_URL_SAFE_NO_PAD, Engine as _};
use rikka_fido::{Registration, RegistrationRequest};
use serde::{Deserialize, Serialize};

use crate::account::Account;
use crate::error::{Error, Result};
use crate::fido2::RegisteredKey;
use crate::registration_options::RegistrationOptions;
use crate::settings::SecondFactorSettings;

const REGISTER_PATH: &str = "core/v4/settings/2fa/register";
const TRANSPORT_USB: &str = "usb";
const MAX_NAME_LEN: usize = 128;

#[derive(Debug, Clone)]
pub struct SecurityKeys {
    account: Account,
}

impl Account {
    #[must_use]
    pub fn security_keys(&self) -> SecurityKeys {
        SecurityKeys {
            account: self.clone(),
        }
    }
}

impl SecurityKeys {
    pub async fn list(&self) -> Result<Vec<RegisteredKey>> {
        let settings = self.account.settings().await?;
        Ok(settings
            .second_factor()
            .map(SecondFactorSettings::registered_keys)
            .unwrap_or_default()
            .to_vec())
    }

    pub async fn challenge(&self, cross_platform: bool) -> Result<SecurityKeyChallenge> {
        self.account
            .api()
            .get(REGISTER_PATH)
            .query(&[("CrossPlatform", i32::from(cross_platform))])
            .send()
            .await
            .map_err(Error::from)
    }

    pub async fn register(
        &self,
        challenge: &SecurityKeyChallenge,
        registration: &Registration,
        name: impl Into<String>,
    ) -> Result<RegisteredKey> {
        let request = RegisterRequest {
            registration_options: &challenge.registration_options,
            client_data: rikka_proton_api::base64::encode(registration.client_data_json()),
            attestation_object: rikka_proton_api::base64::encode(registration.attestation_object()),
            transports: &[TRANSPORT_USB],
            name: validate_name(name.into())?,
        };
        self.account
            .api()
            .post(REGISTER_PATH)
            .json(&request)
            .send_unit()
            .await?;
        self.account.lock_scopes().await?;

        self.list()
            .await?
            .into_iter()
            .find(|key| key.credential_id() == registration.credential_id())
            .ok_or(Error::Incomplete("newly registered security key"))
    }

    pub async fn rename(&self, credential_id: &[u8], name: impl Into<String>) -> Result<()> {
        let request = RenameRequest {
            name: validate_name(name.into())?,
        };
        self.account
            .api()
            .put(key_path(credential_id, "rename"))
            .json(&request)
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn remove<I>(&self, credential_ids: I) -> Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<[u8]>,
    {
        for credential_id in credential_ids {
            self.account
                .api()
                .post(key_path(credential_id.as_ref(), "remove"))
                .json(&RemoveRequest {})
                .send_unit()
                .await?;
        }
        self.account.lock_scopes().await
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SecurityKeyChallenge {
    #[serde(rename = "RegistrationOptions")]
    registration_options: RegistrationOptions,
    #[serde(rename = "RegisteredKeys", default)]
    registered_keys: Vec<RegisteredKey>,
    #[serde(rename = "AttestationFormats", default)]
    attestation_formats: Vec<String>,
}

impl SecurityKeyChallenge {
    #[must_use]
    pub fn registration_options(&self) -> &RegistrationOptions {
        &self.registration_options
    }

    #[must_use]
    pub fn registered_keys(&self) -> &[RegisteredKey] {
        &self.registered_keys
    }

    #[must_use]
    pub fn attestation_formats(&self) -> &[String] {
        &self.attestation_formats
    }

    pub fn registration_request(&self) -> RegistrationRequest {
        let options = &self.registration_options;
        options.registration_request(options.web_origin())
    }
}

fn validate_name(name: String) -> Result<String> {
    let length = name.chars().count();
    match length > MAX_NAME_LEN {
        true => Err(Error::SecurityKeyNameTooLong {
            length,
            limit: MAX_NAME_LEN,
        }),
        false => Ok(name),
    }
}

fn key_path(credential_id: &[u8], action: &str) -> String {
    format!(
        "core/v4/settings/2fa/{}/{action}",
        BASE64_URL_SAFE_NO_PAD.encode(credential_id)
    )
}

#[derive(Debug, Serialize)]
struct RegisterRequest<'a> {
    #[serde(rename = "RegistrationOptions")]
    registration_options: &'a RegistrationOptions,
    #[serde(rename = "ClientData")]
    client_data: String,
    #[serde(rename = "AttestationObject")]
    attestation_object: String,
    #[serde(rename = "Transports")]
    transports: &'a [&'a str],
    #[serde(rename = "Name")]
    name: String,
}

#[derive(Debug, Serialize)]
struct RenameRequest {
    #[serde(rename = "Name")]
    name: String,
}

#[derive(Debug, Serialize)]
struct RemoveRequest {}
