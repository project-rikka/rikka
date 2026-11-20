use rikka_proton_api::offload;
use rikka_proton_pgp::{Secret, Srp};
use serde::Serialize;

use crate::account::Account;
use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Verifier {
    #[serde(rename = "Version")]
    version: u8,
    #[serde(rename = "ModulusID")]
    modulus_id: String,
    #[serde(rename = "Salt")]
    salt: String,
    #[serde(rename = "Verifier")]
    verifier: String,
}

pub(crate) async fn generate(account: &Account, password: &Secret) -> Result<Verifier> {
    let signed_modulus = account.modulus().await?;
    let modulus = signed_modulus.verify()?;
    let password = password.clone();

    let registration = offload(move || {
        Srp::new()
            .register()
            .password(password.expose())
            .modulus(modulus)
            .verifier()
    })
    .await?;

    Ok(Verifier {
        version: registration.version().as_u8(),
        modulus_id: signed_modulus.id().to_owned(),
        salt: registration.salt().to_owned(),
        verifier: registration.verifier().to_owned(),
    })
}
