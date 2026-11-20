use rikka_proton_pgp::{Secret, SrpProofs};
use serde::Serialize;

use crate::account::Account;
use crate::error::{Error, Result};
use crate::fido2::Fido2Assertion;
use crate::srp;

#[derive(Debug, Default)]
pub(crate) struct Reauthentication {
    username: Option<String>,
    password: Option<Secret>,
    totp: Option<String>,
    fido2: Option<Fido2Assertion>,
}

impl Reauthentication {
    pub(crate) fn set_username(&mut self, username: String) {
        self.username = Some(username);
    }

    pub(crate) fn set_password(&mut self, password: Secret) {
        self.password = Some(password);
    }

    pub(crate) fn set_totp(&mut self, code: String) {
        self.totp = Some(code);
    }

    pub(crate) fn set_fido2(&mut self, assertion: Fido2Assertion) {
        self.fido2 = Some(assertion);
    }

    pub(crate) async fn prove(&self, account: &Account) -> Result<Option<SrpProof>> {
        let Some(password) = &self.password else {
            return Ok(None);
        };

        let username = account.resolve_username(self.username.clone()).await?;
        let info = account.srp_info(&username).await?;
        let proofs = srp::proofs(&info, &username, password).await?;

        let request = ProofRequest {
            client_ephemeral: proofs.client_ephemeral().to_owned(),
            client_proof: proofs.client_proof().to_owned(),
            srp_session: info
                .srp_session()
                .ok_or(Error::Incomplete("SRPSession"))?
                .to_owned(),
            totp: self.totp.clone(),
            fido2: self.fido2.clone(),
        };

        Ok(Some(SrpProof { proofs, request }))
    }
}

pub(crate) struct SrpProof {
    proofs: SrpProofs,
    request: ProofRequest,
}

impl SrpProof {
    pub(crate) fn request(&self) -> &ProofRequest {
        &self.request
    }

    pub(crate) fn verify(&self, server_proof: Option<&str>) -> Result<()> {
        srp::verify(&self.proofs, server_proof)
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ProofRequest {
    #[serde(rename = "ClientEphemeral")]
    client_ephemeral: String,
    #[serde(rename = "ClientProof")]
    client_proof: String,
    #[serde(rename = "SRPSession")]
    srp_session: String,
    #[serde(rename = "TwoFactorCode", skip_serializing_if = "Option::is_none")]
    totp: Option<String>,
    #[serde(rename = "FIDO2", skip_serializing_if = "Option::is_none")]
    fido2: Option<Fido2Assertion>,
}

macro_rules! reauthenticated {
    ($builder:ty, $($field:ident).+) => {
        impl $builder {
            pub fn username(mut self, username: impl Into<String>) -> Self {
                self.$($field).+.set_username(username.into());
                self
            }

            pub fn password(mut self, password: impl Into<String>) -> Self {
                self.$($field).+.set_password(::rikka_proton_pgp::Secret::new(password));
                self
            }

            pub fn totp(mut self, code: impl Into<String>) -> Self {
                self.$($field).+.set_totp(code.into());
                self
            }

            pub fn fido2_assertion(mut self, assertion: $crate::fido2::Fido2Assertion) -> Self {
                self.$($field).+.set_fido2(assertion);
                self
            }
        }
    };
}

pub(crate) use reauthenticated;
