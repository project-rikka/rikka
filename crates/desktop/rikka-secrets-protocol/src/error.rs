pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to draw random bytes: {0}")]
    Random(getrandom::Error),
    #[error("peer public key is not in the group")]
    Peer,
    #[error("session key derivation failed")]
    Derivation,
    #[error("secret parameters are not an AES-128 initialization vector")]
    Parameters,
    #[error("secret is not padded AES-128-CBC ciphertext")]
    Padding,
}
