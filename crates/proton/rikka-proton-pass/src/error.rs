use rikka_proton_account::AddressId;
use rikka_proton_pgp::SignatureStatus;

use crate::identifier::{FolderId, GroupId, ItemId, ShareId};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid {subject} protobuf: {reason}")]
    Malformed {
        subject: &'static str,
        reason: String,
    },

    #[error("Pass API request failed")]
    Api(#[from] rikka_proton_api::Error),

    #[error("OpenPGP operation failed")]
    Pgp(#[from] rikka_proton_pgp::Error),

    #[error("failed to read account keys")]
    Account(#[from] rikka_proton_account::Error),

    #[error("{subject} is not valid base64")]
    InvalidBase64 { subject: &'static str },

    #[error("failed to decrypt content")]
    Undecryptable,

    #[error("encryption key is {length} bytes, expected 32")]
    InvalidKeyLength { length: usize },

    #[error("invalid share key signature on {share}: {status}")]
    InvalidShareKeySignature {
        share: ShareId,
        status: SignatureStatus,
    },

    #[error("share {share} has no key for rotation {rotation}")]
    MissingShareKey { share: ShareId, rotation: i64 },

    #[error("share {share} has no keys")]
    NoShareKey { share: ShareId },

    #[error("share {share} has no folder {folder}")]
    MissingFolder { share: ShareId, folder: FolderId },

    #[error("folder cycle at {folder} in share {share}")]
    FolderCycle { share: ShareId, folder: FolderId },

    #[error("share {share} has target type {target}, expected vault")]
    NotAVault {
        share: ShareId,
        target: &'static str,
    },

    #[error("no unlocked key for address {address}")]
    NoAddressKey { address: AddressId },

    #[error("{email} has no Proton encryption key")]
    NoPublicKey { email: String },

    #[error("invalid invite signature from {inviter}: {status}")]
    InvalidInviteSignature {
        inviter: String,
        status: SignatureStatus,
    },

    #[error("invite has no key for rotation {rotation}")]
    MissingInviteKey { rotation: i64 },

    #[error("item {item} has no key for rotation {rotation}")]
    MissingItemKey { item: ItemId, rotation: i64 },

    #[error("unsupported file encryption version {version}")]
    UnsupportedFileVersion { version: i32 },

    #[error("attachment is empty")]
    EmptyFile,

    #[error("attachment size mismatch: read {read} bytes, expected {expected}")]
    FileSizeMismatch { expected: u64, read: u64 },

    #[error("failed to read attachment")]
    Io(#[from] std::io::Error),

    #[error("invalid secure link \"{0}\"")]
    MalformedLink(String),

    #[error("account is not a member of group {group}")]
    NoGroup { group: GroupId },

    #[error("failed to unlock any key of group {group}")]
    GroupKeyLocked { group: GroupId },

    #[error("organization key requires account keys")]
    OrganizationKeyLocked,

    #[error("extra password challenge has no SRP session")]
    MissingSrpSession,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn malformed(subject: &'static str, error: prost::DecodeError) -> Error {
        Error::Malformed {
            subject,
            reason: error.to_string(),
        }
    }
}

impl Error {
    #[must_use]
    pub fn api(&self) -> Option<&rikka_proton_api::Error> {
        match self {
            Error::Api(error) => Some(error),
            Error::Account(error) => error.api(),
            _ => None,
        }
    }
}
