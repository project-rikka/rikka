use rikka_proton_pgp::{Secret, Srp};

use crate::api::{ExtraPasswordProofRequest, SetExtraPasswordRequest};
use crate::client::{PREFIX, PassClient};
use crate::error::{Error, Result};
use rikka_proton_pgp::SrpChallenge;

impl PassClient {
    pub async fn has_extra_password(&self) -> Result<bool> {
        Ok(self
            .api()
            .get(format!("{PREFIX}/user/srp"))
            .send_field_or_default("HasSRP")
            .await?)
    }

    pub async fn set_extra_password(&self, password: impl AsRef<str>) -> Result<()> {
        let signed = self.account().modulus().await?;
        let modulus = signed.verify()?;
        let modulus_id = signed.id().to_owned();
        let password = Secret::new(password.as_ref());

        let verifier = rikka_proton_api::offload(move || {
            Srp::new()
                .register()
                .password(password.expose())
                .modulus(modulus)
                .verifier()
        })
        .await?;

        let request = SetExtraPasswordRequest {
            modulus_id,
            verifier: verifier.verifier().to_owned(),
            salt: verifier.salt().to_owned(),
        };

        self.api()
            .post(format!("{PREFIX}/user/srp"))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn remove_extra_password(&self) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/user/srp"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn unlock_with_extra_password(&self, password: impl AsRef<str>) -> Result<()> {
        let envelope: SrpChallenge = self
            .api()
            .get(format!("{PREFIX}/user/srp/info"))
            .send_field("SRPData")
            .await?;
        let challenge = envelope;
        let password = Secret::new(password.as_ref());

        let session_id = challenge
            .session()
            .ok_or(Error::MissingSrpSession)?
            .to_owned();
        let proofs =
            rikka_proton_api::offload(move || challenge.proofs(None, password.expose())).await?;

        let request = ExtraPasswordProofRequest {
            client_ephemeral: proofs.client_ephemeral().to_owned(),
            client_proof: proofs.client_proof().to_owned(),
            session_id,
        };
        let response: String = self
            .api()
            .post(format!("{PREFIX}/user/srp/auth"))
            .json(&request)
            .send_field_or_default("ServerProof")
            .await?;

        Ok(proofs.verify_server_proof(&response)?)
    }
}
