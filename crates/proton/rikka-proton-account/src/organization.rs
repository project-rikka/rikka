use rikka_proton_api::{Page, Paging, offload, parallel_map_each};
use rikka_proton_pgp::{KeyRing, LockedKey, Pgp, PrivateKey, SecretBytes};
use serde::Deserialize;

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::address::Address;
use crate::address_key::AddressKey;
use crate::auth_device::AuthDevice;
use crate::auth_devices::MemberKeys;
use crate::error::{Error, Result};
use crate::identifier::{KeyId, UserId};
use crate::key_token::KeyToken;
use crate::unprivatization::UnprivatizationState;

const ORGANIZATION_KEYS_PATH: &str = "core/v4/organizations/keys";
pub(crate) const ORGANIZATION_TOKEN_CONTEXT: &str = "account.key-token.organization";
pub(crate) const ROLE_ORGANIZATION_ADMIN: i32 = 2;
const MEMBER_ADDRESSES_PAGE_SIZE: u32 = 150;

impl Account {
    pub async fn organization_key(&self, keys: &AccountKeys) -> Result<PrivateKey> {
        let key_response: OrganizationKeyResponse =
            self.api().get(ORGANIZATION_KEYS_PATH).send().await?;
        let pgp = *self.pgp();
        let user_keys = keys.user_keys().clone();
        let passphrase = keys.passphrase().cloned();

        offload(move || key_response.unlock(&pgp, &user_keys, passphrase.as_ref())).await
    }

    pub(crate) async fn passwordless_organization_token(
        &self,
        keys: &AccountKeys,
    ) -> Result<Option<String>> {
        let key_response: OrganizationKeyResponse =
            self.api().get(ORGANIZATION_KEYS_PATH).send().await?;
        let Some(token) = key_response
            .token
            .clone()
            .filter(|_| key_response.signature.is_some())
        else {
            return Ok(None);
        };
        let pgp = *self.pgp();
        let user_keys = keys.user_keys().clone();
        let passphrase = keys.passphrase().cloned();
        offload(move || key_response.unlock(&pgp, &user_keys, passphrase.as_ref())).await?;

        Ok(Some(token))
    }

    pub async fn member_keys(
        &self,
        organization_key: &PrivateKey,
        member: &UserId,
    ) -> Result<MemberKeys> {
        let (member_response, addresses) =
            futures_util::try_join!(self.member(member), self.member_addresses(member))?;

        let pgp = *self.pgp();
        let organization = organization_key.clone();
        let user_keys: Vec<(KeyId, PrivateKey)> =
            parallel_map_each(member_response.keys, move |key| {
                let result = unlock_member_key(&pgp, &key, &organization);
                (key.id, result)
            })
            .await
            .into_iter()
            .filter_map(|(id, result)| match result {
                Ok(unlocked) => Some((id, unlocked)),
                Err(error) => {
                    tracing::warn!(key = %id, %error, "failed to unlock member user key");
                    None
                }
            })
            .collect();
        if user_keys.is_empty() {
            return Err(Error::NoKeyUnlocked);
        }

        let user_ring = user_keys.iter().fold(KeyRing::new(), |ring, (_, key)| {
            ring.with_private(key.clone())
        });
        let active_address_keys: Vec<AddressKey> = addresses
            .iter()
            .flat_map(Address::keys)
            .filter(|key| key.is_active())
            .cloned()
            .collect();
        let organization = organization_key.clone();
        let address_keys = parallel_map_each(active_address_keys, move |key| {
            let result = unlock_member_address_key(&pgp, &key, &user_ring, &organization);
            (key.id().clone(), result)
        })
        .await;

        let member_keys = address_keys
            .into_iter()
            .filter_map(|(id, result)| match result {
                Ok(unlocked) => Some(unlocked),
                Err(error) => {
                    tracing::warn!(key = %id, %error, "failed to unlock member address key");
                    None
                }
            })
            .fold(MemberKeys::new(), MemberKeys::address_key);

        Ok(user_keys
            .into_iter()
            .fold(member_keys, |member, (id, key)| member.user_key(id, key)))
    }

    pub async fn approve_member_device_as_admin(
        &self,
        keys: &AccountKeys,
        device: &AuthDevice,
        confirmation_code: &str,
    ) -> Result<()> {
        let member = device.member_id().ok_or(Error::Incomplete("MemberID"))?;
        let organization_key = self.organization_key(keys).await?;
        if self
            .member(member)
            .await?
            .unprivatization
            .as_ref()
            .is_some_and(MemberUnprivatization::is_pending_manual_approval)
        {
            self.unprivatize_member(&organization_key, member).await?;
        }
        let member_keys = self.member_keys(&organization_key, member).await?;

        self.approve_member_device(device, confirmation_code, &member_keys)
            .await
    }

    pub(crate) async fn member(&self, member: &UserId) -> Result<Member> {
        Ok(self
            .api()
            .get(format!("core/v4/members/{member}"))
            .send_field("Member")
            .await?)
    }

    pub(crate) async fn member_addresses(&self, member: &UserId) -> Result<Vec<Address>> {
        Ok(self
            .api()
            .get(format!("core/v4/members/{member}/addresses"))
            .pages(Paging::numbered(MEMBER_ADDRESSES_PAGE_SIZE))
            .collect(|response: MemberAddressesResponse| {
                Page::new(response.addresses)
                    .total(response.total.and_then(|total| usize::try_from(total).ok()))
            })
            .await?)
    }
}

fn unlock_member_key(pgp: &Pgp, key: &MemberKey, organization: &PrivateKey) -> Result<PrivateKey> {
    let token = key
        .token
        .as_deref()
        .ok_or_else(|| Error::NoKeyToken(key.id.clone()))?;
    let secret = decrypt_organization_token(pgp, &key.id, token, organization)?;

    Ok(LockedKey::import(&key.private_key)?.unlock(&secret)?)
}

fn unlock_member_address_key(
    pgp: &Pgp,
    key: &AddressKey,
    user_ring: &KeyRing,
    organization: &PrivateKey,
) -> Result<PrivateKey> {
    let token = key
        .token()
        .ok_or_else(|| Error::NoKeyToken(key.id().clone()))?;
    let secret = match key.signature() {
        Some(signature) => KeyToken::new(token, signature).open(pgp, user_ring)?,
        None => decrypt_organization_token(pgp, key.id(), token, organization)?,
    };

    Ok(key.to_locked_key()?.unlock(&secret)?)
}

fn decrypt_organization_token(
    pgp: &Pgp,
    id: &KeyId,
    token: &str,
    organization: &PrivateKey,
) -> Result<SecretBytes> {
    let decrypted = pgp
        .decrypt()
        .with_key(organization.clone())
        .verified_by(organization.public_key())
        .message(token)?;

    if !decrypted.signature().is_valid() {
        return Err(Error::UnsignedMemberToken {
            key: id.clone(),
            status: Box::new(decrypted.signature().clone()),
        });
    }

    Ok(SecretBytes::new(decrypted.into_data()))
}

#[derive(Debug, Deserialize)]
struct OrganizationKeyResponse {
    #[serde(rename = "PrivateKey", default)]
    private_key: Option<String>,
    #[serde(rename = "LegacyPrivateKey", default)]
    legacy_private_key: Option<String>,
    #[serde(rename = "Token", default)]
    token: Option<String>,
    #[serde(rename = "Signature", default)]
    signature: Option<String>,
    #[serde(rename = "Passwordless", default)]
    passwordless: bool,
}

impl OrganizationKeyResponse {
    fn unlock(
        &self,
        pgp: &Pgp,
        user_keys: &KeyRing,
        passphrase: Option<&SecretBytes>,
    ) -> Result<PrivateKey> {
        if let Some(legacy) = &self.legacy_private_key {
            let passphrase = passphrase.ok_or(Error::NoPassphrase)?;
            return Ok(LockedKey::import(legacy)?.unlock(passphrase)?);
        }
        let armored = self
            .private_key
            .as_deref()
            .ok_or(Error::NoOrganizationKey)?;

        let secret = match (&self.token, &self.signature) {
            (Some(token), Some(signature)) => KeyToken::new(token, signature)
                .context(ORGANIZATION_TOKEN_CONTEXT)
                .open(pgp, user_keys)?,
            _ if self.passwordless => return Err(Error::NoOrganizationKey),
            _ => passphrase.ok_or(Error::NoPassphrase)?.clone(),
        };

        Ok(LockedKey::import(armored)?.unlock(&secret)?)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Member {
    #[serde(rename = "Keys", default)]
    keys: Vec<MemberKey>,
    #[serde(rename = "Unprivatization", default)]
    pub(crate) unprivatization: Option<MemberUnprivatization>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemberUnprivatization {
    #[serde(rename = "State", default)]
    state: UnprivatizationState,
    #[serde(rename = "PrivateIntent", default)]
    private_intent: bool,
    #[serde(rename = "InvitationData", default)]
    invitation_data: Option<String>,
    #[serde(rename = "InvitationSignature", default)]
    invitation_signature: Option<String>,
    #[serde(rename = "ActivationToken", default)]
    pub(crate) activation_token: Option<String>,
    #[serde(rename = "PrivateKeys", default)]
    pub(crate) private_keys: Option<Vec<String>>,
}

impl MemberUnprivatization {
    pub(crate) fn is_pending_manual_approval(&self) -> bool {
        self.state == UnprivatizationState::Ready
            && !self.private_intent
            && self.invitation_data.is_none()
            && self.invitation_signature.is_none()
            && self.activation_token.is_some()
            && self
                .private_keys
                .as_ref()
                .is_some_and(|keys| !keys.is_empty())
    }
}

#[derive(Debug, Deserialize)]
struct MemberKey {
    #[serde(rename = "ID")]
    id: KeyId,
    #[serde(rename = "PrivateKey")]
    private_key: String,
    #[serde(rename = "Token", default)]
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MemberAddressesResponse {
    #[serde(rename = "Addresses", default)]
    addresses: Vec<Address>,
    #[serde(rename = "Total", default)]
    total: Option<i64>,
}
