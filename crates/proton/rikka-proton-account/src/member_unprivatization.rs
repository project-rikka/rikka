use rikka_proton_api::offload;
use rikka_proton_pgp::{KeyRing, LockedKey, Pgp, PrivateKey, Secret};
use serde::Serialize;

use crate::account::Account;
use crate::address::Address;
use crate::error::{Error, Result};
use crate::identifier::{KeyId, UserId};
use crate::organization::MemberUnprivatization;
use crate::resealed_token::reseal_token;

const TOKEN_LEN: usize = 32;

impl Account {
    pub async fn unprivatize_member(
        &self,
        organization_key: &PrivateKey,
        member: &UserId,
    ) -> Result<()> {
        let (member_keys, addresses) =
            futures_util::try_join!(self.member(member), self.member_addresses(member))?;
        let pending = member_keys
            .unprivatization
            .filter(MemberUnprivatization::is_pending_manual_approval)
            .ok_or(Error::Incomplete("Unprivatization"))?;

        let pgp = *self.pgp();
        let organization = organization_key.clone();
        let request =
            offload(move || build_request(&pgp, &organization, &pending, &addresses)).await?;

        self.api()
            .post(format!("core/v4/members/{member}/keys/unprivatize"))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }
}

fn build_request(
    pgp: &Pgp,
    organization: &PrivateKey,
    pending: &MemberUnprivatization,
    addresses: &[Address],
) -> Result<UnprivatizeRequest> {
    let activation = pending
        .activation_token
        .as_deref()
        .ok_or(Error::Incomplete("ActivationToken"))?;
    let decrypted = pgp
        .decrypt()
        .with_key(organization.clone())
        .message(activation)?;
    let token = Secret::new(
        String::from_utf8(decrypted.into_data()).map_err(|_| Error::MalformedKeyToken)?,
    );

    let user_keys: Vec<PrivateKey> = pending
        .private_keys
        .iter()
        .flatten()
        .map(|armored| Ok(LockedKey::import(armored)?.unlock(token.expose())?))
        .collect::<Result<_>>()?;
    let member_ring = user_keys
        .iter()
        .fold(KeyRing::new(), |ring, key| ring.with_private(key.clone()));

    let new_token = Secret::random_hex(TOKEN_LEN);
    let organization_token = pgp
        .encrypt()
        .to(organization.public_key())
        .signed_by(organization.clone())
        .as_utf8()
        .armored_message(new_token.expose())?;
    let relocked_user_keys = user_keys
        .iter()
        .map(|key| {
            Ok(UnprivatizedUserKey {
                private_key: key.lock(new_token.expose())?.export_armored()?,
                token: organization_token.clone(),
            })
        })
        .collect::<Result<_>>()?;

    let address_keys = addresses
        .iter()
        .flat_map(|address| address.keys())
        .map(|key| {
            let token = key
                .token()
                .ok_or_else(|| Error::NoKeyToken(key.id().clone()))?;
            let resealed = reseal_token(
                pgp,
                token,
                &member_ring,
                &organization.public_key(),
                organization,
                None,
            )?;
            Ok(UnprivatizedAddressKey {
                id: key.id().clone(),
                signature: resealed.signature,
                key_packet: resealed.key_packet,
            })
        })
        .collect::<Result<_>>()?;

    Ok(UnprivatizeRequest {
        user_keys: relocked_user_keys,
        address_keys,
    })
}

#[derive(Debug, Serialize)]
struct UnprivatizeRequest {
    #[serde(rename = "UserKeys")]
    user_keys: Vec<UnprivatizedUserKey>,
    #[serde(rename = "AddressKeys")]
    address_keys: Vec<UnprivatizedAddressKey>,
}

#[derive(Debug, Serialize)]
struct UnprivatizedUserKey {
    #[serde(rename = "OrgPrivateKey")]
    private_key: String,
    #[serde(rename = "OrgToken")]
    token: String,
}

#[derive(Debug, Serialize)]
struct UnprivatizedAddressKey {
    #[serde(rename = "AddressKeyID")]
    id: KeyId,
    #[serde(rename = "OrgSignature")]
    signature: String,
    #[serde(rename = "OrgTokenKeyPacket")]
    key_packet: String,
}
