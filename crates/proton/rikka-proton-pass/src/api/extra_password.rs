use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetExtraPasswordRequest {
    #[serde(rename = "SrpModulusID")]
    pub(crate) modulus_id: String,
    #[serde(rename = "SrpVerifier")]
    pub(crate) verifier: String,
    #[serde(rename = "SrpSalt")]
    pub(crate) salt: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ExtraPasswordProofRequest {
    #[serde(rename = "ClientEphemeral")]
    pub(crate) client_ephemeral: String,
    #[serde(rename = "ClientProof")]
    pub(crate) client_proof: String,
    #[serde(rename = "SrpSessionID")]
    pub(crate) session_id: String,
}
