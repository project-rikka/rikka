use rikka_proton_api::offload;
use rikka_proton_pgp::{Secret, SecretBytes, Srp, random_bytes};

use crate::error::Result;

const KEY_SALT_LEN: usize = 16;

pub(crate) struct SaltedPassphrase {
    pub(crate) salt: String,
    pub(crate) passphrase: SecretBytes,
}

pub(crate) async fn derive(password: Secret, salt: Vec<u8>) -> Result<SecretBytes> {
    let hashed = offload(move || Srp::new().mailbox_passphrase(password.expose(), salt)).await?;

    Ok(SecretBytes::new(hashed.as_bytes()))
}

pub(crate) async fn derive_with_new_salt(password: Secret) -> Result<SaltedPassphrase> {
    let salt = random_bytes::<KEY_SALT_LEN>();
    let passphrase = derive(password, salt.to_vec()).await?;

    Ok(SaltedPassphrase {
        salt: rikka_proton_api::base64::encode(salt),
        passphrase,
    })
}
