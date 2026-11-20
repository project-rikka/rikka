use rikka_proton_api::offload;
use rikka_proton_pgp::{Secret, SrpProofs};

use crate::auth_info::AuthInfo;
use crate::error::{Error, Result};

pub(crate) async fn proofs(
    info: &AuthInfo,
    username: &str,
    password: &Secret,
) -> Result<SrpProofs> {
    let challenge = info.challenge().clone();
    let username = username.to_owned();
    let password = password.clone();

    Ok(offload(move || challenge.proofs(Some(&username), password.expose())).await?)
}

pub(crate) fn verify(proofs: &SrpProofs, server_proof: Option<&str>) -> Result<()> {
    let server_proof = server_proof.ok_or(Error::Incomplete("ServerProof"))?;

    Ok(proofs.verify_server_proof(server_proof)?)
}
