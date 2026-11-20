use proton_srp::SRPProofB64;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct SrpProofs(SRPProofB64);

impl SrpProofs {
    #[must_use]
    pub fn client_ephemeral(&self) -> &str {
        &self.0.client_ephemeral
    }

    #[must_use]
    pub fn client_proof(&self) -> &str {
        &self.0.client_proof
    }

    pub fn verify_server_proof(&self, server_proof: &str) -> Result<()> {
        match self.0.compare_server_proof(server_proof) {
            true => Ok(()),
            false => Err(Error::ServerProof),
        }
    }

    pub(crate) fn new(proofs: SRPProofB64) -> Self {
        SrpProofs(proofs)
    }
}
