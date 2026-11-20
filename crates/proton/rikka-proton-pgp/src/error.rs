use std::error::Error as StdError;

use proton_rpgp::{
    ArmorError, DecryptionError, EncryptionError, KeyOperationError, MessageVerificationError,
};

use crate::signature_status::SignatureStatus;

pub type Cause = Box<dyn StdError + Send + Sync>;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("malformed OpenPGP data")]
    Malformed(#[source] Cause),

    #[error("private key is locked")]
    KeyLocked,

    #[error("incorrect key passphrase")]
    WrongPassphrase,

    #[error("key cannot be used for this operation")]
    KeyUnusable(#[source] Cause),

    #[error("key generation failed")]
    KeyGeneration(#[source] Cause),
    #[error("key modification failed")]
    KeyModification(#[source] Cause),

    #[error("stream I/O failed")]
    Io(#[source] std::io::Error),

    #[error("no recipient key, password or session key")]
    NoRecipient,

    #[error("encryption failed")]
    EncryptionFailed(#[source] Cause),

    #[error("decryption failed")]
    DecryptionFailed(#[source] Cause),

    #[error("signing failed")]
    SigningFailed(#[source] Cause),

    #[error("invalid signature: {0}")]
    SignatureInvalid(SignatureStatus),

    #[error("invalid SRP modulus")]
    InvalidModulus(#[source] Cause),

    #[error("invalid SRP server proof")]
    ServerProof,

    #[error("SRP exchange failed")]
    SrpFailed(#[source] Cause),

    #[error("password hashing failed")]
    PasswordHash(#[source] Cause),

    #[error("{0}")]
    Invalid(&'static str),
}

impl From<proton_rpgp::Error> for Error {
    fn from(error: proton_rpgp::Error) -> Self {
        match error {
            proton_rpgp::Error::KeyOperation(KeyOperationError::Locked) => Error::KeyLocked,
            proton_rpgp::Error::KeyOperation(KeyOperationError::Unlock(..)) => {
                Error::WrongPassphrase
            }
            proton_rpgp::Error::KeyOperation(operation) => Error::Malformed(Box::new(operation)),
            proton_rpgp::Error::KeyGeneration(generation) => {
                Error::KeyGeneration(Box::new(generation))
            }
            proton_rpgp::Error::KeyValidation(validation) => {
                Error::KeyUnusable(Box::new(validation))
            }
            proton_rpgp::Error::KeyModification(modification) => {
                Error::KeyUnusable(Box::new(modification))
            }
            proton_rpgp::Error::Signing(signing) => Error::SigningFailed(Box::new(signing)),
            proton_rpgp::Error::Decryption(DecryptionError::LockedKey) => Error::KeyLocked,
            proton_rpgp::Error::Decryption(decryption) => {
                Error::DecryptionFailed(Box::new(decryption))
            }
            proton_rpgp::Error::Encryption(EncryptionError::MissingEncryptionTools) => {
                Error::NoRecipient
            }
            proton_rpgp::Error::Encryption(encryption) => {
                Error::EncryptionFailed(Box::new(encryption))
            }
            proton_rpgp::Error::Verification(verification) => {
                Error::Malformed(Box::new(verification))
            }
            proton_rpgp::Error::Armor(armor) => Error::Malformed(Box::new(armor)),
            proton_rpgp::Error::EncryptedMessage(message) => Error::Malformed(Box::new(message)),
            proton_rpgp::Error::VerificationResultUtility(utility) => {
                Error::Malformed(Box::new(utility))
            }
        }
    }
}

impl From<ArmorError> for Error {
    fn from(error: ArmorError) -> Self {
        Error::Malformed(Box::new(error))
    }
}

impl From<MessageVerificationError> for Error {
    fn from(error: MessageVerificationError) -> Self {
        Error::Malformed(Box::new(error))
    }
}

impl From<proton_srp::SRPError> for Error {
    fn from(error: proton_srp::SRPError) -> Self {
        match error {
            proton_srp::SRPError::ModulusVerify(verify) => Error::InvalidModulus(Box::new(verify)),
            proton_srp::SRPError::InvalidModulus(reason) => Error::Invalid(reason),
            other => Error::SrpFailed(Box::new(other)),
        }
    }
}

impl From<proton_srp::MailboxHashError> for Error {
    fn from(error: proton_srp::MailboxHashError) -> Self {
        match error {
            proton_srp::MailboxHashError::InvalidSalt => {
                Error::Invalid("key salt must be 16 bytes")
            }
            other => Error::PasswordHash(Box::new(other)),
        }
    }
}
