use rikka_proton_pgp::{Pgp, PrivateKey, SessionKey};

use crate::crypto;
use crate::error::{Error, Result};
use crate::upload::wire::VerificationResponse;

#[derive(Clone)]
pub(super) struct Verifier {
    code: Vec<u8>,
    key: SessionKey,
}

impl Verifier {
    pub(super) fn open(
        pgp: &Pgp,
        node_key: &PrivateKey,
        response: &VerificationResponse,
    ) -> Result<Verifier> {
        let code = rikka_proton_api::base64::decode(&response.code)
            .map_err(|_| Error::NotBase64("verification code"))?;
        let packet = rikka_proton_api::base64::decode(&response.content_key_packet)
            .map_err(|_| Error::NotBase64("verification content key packet"))?;
        let key = crypto::decrypt_session_key(pgp, std::slice::from_ref(node_key), &packet)?;

        Ok(Verifier { code, key })
    }

    pub(super) fn token(&self, pgp: &Pgp, index: u32, sealed: &[u8]) -> Result<Vec<u8>> {
        crypto::decrypt_block(pgp, &self.key, sealed)
            .map_err(|_| Error::BlockVerification { index })?;

        Ok(self
            .code
            .iter()
            .enumerate()
            .map(|(offset, byte)| byte ^ sealed.get(offset).copied().unwrap_or(0))
            .collect())
    }
}
