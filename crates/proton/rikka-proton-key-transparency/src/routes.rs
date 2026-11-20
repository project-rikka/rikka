use rikka_proton_account::{AddressId, KeyId, SignedKeyList};
use rikka_proton_api::Api;
use serde::{Deserialize, Serialize};

use crate::epoch::Epoch;
use crate::error::{Error, Result};
use crate::identifier::Identifier;
use crate::proof::Proof;
use crate::verified_epoch::SignedVerifiedEpoch;

const ADDRESS_STATUS_DISABLED: i32 = 0;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AuditedAddress {
    #[serde(rename = "ID")]
    pub(crate) id: AddressId,
    #[serde(rename = "Email")]
    pub(crate) email: String,
    #[serde(rename = "Status", default)]
    pub(crate) status: i32,
    #[serde(rename = "Keys", default)]
    pub(crate) keys: Vec<AuditedKey>,
    #[serde(rename = "SignedKeyList", default)]
    pub(crate) signed_key_list: Option<SignedKeyList>,
}

impl AuditedAddress {
    pub(crate) fn is_disabled(&self) -> bool {
        self.status == ADDRESS_STATUS_DISABLED
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AuditedKey {
    #[serde(rename = "ID")]
    pub(crate) id: KeyId,
    #[serde(rename = "Flags", default)]
    pub(crate) flags: i32,
    #[serde(rename = "Primary", default, with = "rikka_proton_api::flag")]
    pub(crate) primary: bool,
}

#[derive(Serialize)]
struct Publication<'a> {
    #[serde(rename = "AddressID")]
    address_id: &'a AddressId,
    #[serde(rename = "SignedKeyList")]
    signed_key_list: SignedKeyListBody<'a>,
}

#[derive(Serialize)]
struct SignedKeyListBody<'a> {
    #[serde(rename = "Data")]
    data: &'a str,
    #[serde(rename = "Signature")]
    signature: &'a str,
}

#[derive(Serialize)]
struct Resignature<'a> {
    #[serde(rename = "AddressID")]
    address_id: &'a AddressId,
    #[serde(rename = "Revision")]
    revision: u32,
    #[serde(rename = "Signature")]
    signature: &'a str,
}

pub(crate) async fn latest_epoch(api: &Api) -> Result<Epoch> {
    let response: Vec<Epoch> = api
        .get("kt/v1/epochs")
        .send_field_or_default("Epochs")
        .await?;
    response.into_iter().next().ok_or(Error::NoEpoch)
}

pub(crate) async fn epoch(api: &Api, id: u32) -> Result<Epoch> {
    Ok(api.get(format!("kt/v1/epochs/{id}")).send().await?)
}

pub(crate) async fn proof(
    api: &Api,
    epoch_id: u32,
    identifier: &Identifier,
    revision: u32,
) -> Result<Proof> {
    Ok(api
        .get(format!("kt/v1/epochs/{epoch_id}/proof"))
        .query(&[
            ("Identifier", identifier.as_str()),
            ("Revision", &revision.to_string()),
        ])
        .send_field("Proof")
        .await?)
}

pub(crate) async fn signed_key_lists(
    api: &Api,
    after_revision: u32,
    identifier: &Identifier,
) -> Result<Vec<SignedKeyList>> {
    Ok(api
        .get("core/v4/keys/signedkeylists")
        .query(&[
            ("AfterRevision", after_revision.to_string().as_str()),
            ("Identifier", identifier.as_str()),
        ])
        .send_field_or_default("SignedKeyLists")
        .await?)
}

pub(crate) async fn signed_key_list(
    api: &Api,
    revision: u32,
    identifier: &Identifier,
) -> Result<Option<SignedKeyList>> {
    let response = api
        .get("core/v4/keys/signedkeylist")
        .query(&[
            ("Revision", revision.to_string().as_str()),
            ("Identifier", identifier.as_str()),
        ])
        .send_field("SignedKeyList")
        .await;
    unless_unprocessable(response)
}

pub(crate) async fn publish_signed_key_list(
    api: &Api,
    address_id: &AddressId,
    data: &str,
    signature: &str,
) -> Result<SignedKeyList> {
    Ok(api
        .post("core/v4/keys/signedkeylists")
        .json(&Publication {
            address_id,
            signed_key_list: SignedKeyListBody { data, signature },
        })
        .send_field("SignedKeyList")
        .await?)
}

pub(crate) async fn replace_signature(
    api: &Api,
    address_id: &AddressId,
    revision: u32,
    signature: &str,
) -> Result<()> {
    api.put("core/v4/keys/signedkeylists/signature")
        .json(&Resignature {
            address_id,
            revision,
            signature,
        })
        .send_unit()
        .await?;
    Ok(())
}

pub(crate) async fn verified_epoch(
    api: &Api,
    address_id: &AddressId,
) -> Result<Option<SignedVerifiedEpoch>> {
    let response = api
        .get(format!("kt/v1/verifiedepoch/{address_id}"))
        .send::<SignedVerifiedEpoch>()
        .await;
    unless_unprocessable(response)
}

pub(crate) async fn upload_verified_epoch(
    api: &Api,
    address_id: &AddressId,
    verified_epoch: &SignedVerifiedEpoch,
) -> Result<()> {
    api.put(format!("kt/v1/verifiedepoch/{address_id}"))
        .json(verified_epoch)
        .send_unit()
        .await?;
    Ok(())
}

pub(crate) async fn address(api: &Api, address_id: &AddressId) -> Result<AuditedAddress> {
    Ok(api
        .get(format!("core/v4/addresses/{address_id}"))
        .send_field("Address")
        .await?)
}

fn unless_unprocessable<T>(response: rikka_proton_api::Result<T>) -> Result<Option<T>> {
    match response.map_err(Error::from) {
        Ok(response) => Ok(Some(response)),
        Err(error) if error.is_unprocessable() => Ok(None),
        Err(error) => Err(error),
    }
}
