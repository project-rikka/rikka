use rikka_proton_account::{AddressKey, KeyToken, UserKeys};
use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey};

use crate::error::{Error, Result};
use crate::identifier::GroupId;

const ADDRESS_TOKEN: &str = "account.key-token.address";

pub(crate) enum GroupKeyUnlocker {
    Owner(UserKeys),
    Administrator(PrivateKey),
}

pub(crate) fn unlock_group_keys(
    pgp: &Pgp,
    group: &GroupId,
    keys: &[AddressKey],
    unlocker: &GroupKeyUnlocker,
) -> Result<UserKeys> {
    let (ring, context) = match unlocker {
        GroupKeyUnlocker::Owner(user) => (user.ring().clone(), Some(ADDRESS_TOKEN)),
        GroupKeyUnlocker::Administrator(organization) => {
            (KeyRing::new().with_private(organization.clone()), None)
        }
    };

    let mut ordered: Vec<&AddressKey> = keys.iter().filter(|key| key.is_active()).collect();
    ordered.sort_by_key(|key| !key.is_primary());

    let unlocked: Vec<PrivateKey> = ordered
        .into_iter()
        .filter_map(|key| {
            let unlocked = unlock_group_key(pgp, &ring, context, key);
            if let Err(error) = &unlocked {
                tracing::warn!(group = %group, key = %key.id(), %error, "failed to unlock group key");
            }
            unlocked.ok()
        })
        .collect();

    let signer = unlocked
        .first()
        .cloned()
        .ok_or_else(|| Error::GroupKeyLocked {
            group: group.clone(),
        })?;

    Ok(UserKeys::new(
        KeyRing::new().with_private_keys(unlocked),
        signer,
    ))
}

fn unlock_group_key(
    pgp: &Pgp,
    ring: &KeyRing,
    context: Option<&str>,
    key: &AddressKey,
) -> Result<PrivateKey> {
    let (Some(token), Some(signature)) = (key.token(), key.signature()) else {
        return Err(rikka_proton_account::Error::NoKeyToken(key.id().clone()).into());
    };

    let mut token = KeyToken::new(token, signature);
    if let Some(context) = context {
        token = token.context(context);
    }
    let passphrase = token.open(pgp, ring)?;

    Ok(key.to_locked_key()?.unlock(passphrase.expose())?)
}
