use rikka_proton_account::SignedModulus;
use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey, Secret, SessionKey, Srp};

use crate::error::Result;
use crate::package::OutsideAuth;

const TOKEN_BYTES: usize = 32;

pub(crate) struct EncryptedBody {
    session_key: SessionKey,
    data_packet: Vec<u8>,
}

impl EncryptedBody {
    pub(crate) fn session_key(&self) -> &SessionKey {
        &self.session_key
    }

    pub(crate) fn data_packet(&self) -> &[u8] {
        &self.data_packet
    }
}

pub(crate) fn encrypt_body(pgp: &Pgp, signer: &PrivateKey, body: &str) -> Result<EncryptedBody> {
    let split = pgp
        .encrypt()
        .to(signer.public_key())
        .signed_by(signer.clone())
        .as_utf8()
        .split(body)?;
    let (_, data_packet, session_key) = split.into_parts();

    Ok(EncryptedBody {
        session_key,
        data_packet,
    })
}

pub(crate) fn key_packet_for(
    pgp: &Pgp,
    recipient: &PublicKey,
    session_key: &SessionKey,
) -> Result<Vec<u8>> {
    Ok(pgp
        .encrypt()
        .to(recipient.clone())
        .key_packets(session_key)?)
}

pub(crate) fn key_packet_with_password(
    pgp: &Pgp,
    password: &str,
    session_key: &SessionKey,
) -> Result<Vec<u8>> {
    Ok(pgp
        .encrypt()
        .with_password(password)
        .key_packets(session_key)?)
}

#[derive(Clone)]
pub(crate) struct Challenge {
    token: Secret,
    encrypted_token: String,
    auth: OutsideAuth,
}

impl Challenge {
    pub(crate) fn token(&self) -> &str {
        self.token.expose()
    }

    pub(crate) fn encrypted_token(&self) -> &str {
        &self.encrypted_token
    }

    pub(crate) fn auth(&self) -> &OutsideAuth {
        &self.auth
    }
}

pub(crate) fn outside_challenge(
    pgp: &Pgp,
    password: &str,
    modulus: &SignedModulus,
) -> Result<Challenge> {
    let token = Secret::random_base64(TOKEN_BYTES);

    let encrypted_token = pgp
        .encrypt()
        .with_password(password)
        .as_utf8()
        .armored_message(token.expose())?;

    let verifier = Srp::new()
        .register()
        .password(password)
        .modulus(modulus.verify()?)
        .verifier()?;

    Ok(Challenge {
        token,
        encrypted_token,
        auth: OutsideAuth::new(
            verifier.version().as_u8(),
            verifier.modulus_id().unwrap_or_default(),
            verifier.salt(),
            verifier.verifier(),
        ),
    })
}
