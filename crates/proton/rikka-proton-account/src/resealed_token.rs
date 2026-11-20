use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey, PublicKey, SignatureContext, unarmor};

use crate::error::Result;

pub(crate) struct ResealedToken {
    pub(crate) key_packet: String,
    pub(crate) signature: String,
}

pub(crate) fn reseal_token(
    pgp: &Pgp,
    token: &str,
    decryption_keys: &KeyRing,
    recipient: &PublicKey,
    signer: &PrivateKey,
    context: Option<SignatureContext>,
) -> Result<ResealedToken> {
    let message = unarmor(token)?;
    let session_key = pgp
        .decrypt()
        .with_ring(decryption_keys)
        .session_key(&message)?;
    let decrypted = pgp
        .decrypt()
        .with_session_key(session_key.clone())
        .message(&message)?;

    let key_packets = pgp
        .encrypt()
        .to(recipient.clone())
        .key_packets(&session_key)?;
    let signing = pgp.sign().with_key(signer.clone());
    let signing = match context {
        Some(context) => signing.with_context(context),
        None => signing,
    };

    Ok(ResealedToken {
        key_packet: rikka_proton_api::base64::encode(key_packets),
        signature: signing.armored_detached(decrypted.data())?,
    })
}
