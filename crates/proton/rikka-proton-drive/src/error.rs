use rikka_proton_pgp::SignatureStatus;

use crate::identifier::{DeviceId, LinkId, RevisionId, ShareId, ThumbnailId};

pub type Cause = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Api(#[from] rikka_proton_api::Error),

    #[error(transparent)]
    Pgp(#[from] rikka_proton_pgp::Error),

    #[error(transparent)]
    Account(#[from] rikka_proton_account::Error),

    #[error("block transfer failed: {0}")]
    Transfer(#[source] Cause),

    #[error("share {0} has no address")]
    NoShareAddress(ShareId),

    #[error("API response has no {0}")]
    Incomplete(&'static str),

    #[error("invalid {field} signature: {status}")]
    Signature {
        field: &'static str,
        status: Box<SignatureStatus>,
    },

    #[error("link {0} is not a folder")]
    NotAFolder(LinkId),

    #[error("link {0} is not a file")]
    NotAFile(LinkId),

    #[error("link {0} is neither a folder nor a file")]
    UnknownNode(LinkId),

    #[error("folder {0} has no hash key")]
    NoHashKey(LinkId),

    #[error("share {0} root has changed")]
    MovedRoot(ShareId),

    #[error("link {0} not found in volume")]
    LinkNotFound(LinkId),

    #[error("failed to read thumbnail {id}: {reason}")]
    ThumbnailRefused { id: ThumbnailId, reason: String },

    #[error("file {0} has no active revision")]
    NoRevision(LinkId),

    #[error("{0} is not valid base64")]
    NotBase64(&'static str),

    #[error("{0} is not valid UTF-8")]
    NotText(&'static str),

    #[error("invalid extended attributes JSON")]
    Attributes(#[source] serde_json::Error),

    #[error("block {index} hash mismatch")]
    BlockCorrupted { index: u32 },

    #[error("block {index} has no download URL")]
    BlockUnreachable { index: u32 },

    #[error("no upload URL for block {index}")]
    NoUploadLink { index: u32 },

    #[error("file I/O failed")]
    Io(#[source] std::io::Error),

    #[error("link {link} or its draft already exists")]
    Conflict {
        link: LinkId,
        draft: Option<RevisionId>,
    },

    #[error("API rejected item: {message} ({code})")]
    Refused {
        code: rikka_proton_api::ResponseCode,
        message: String,
    },

    #[error("no available name for {0}")]
    NoAvailableName(String),

    #[error("{0} has no Proton key for invitation")]
    NoPublicKey(String),

    #[error("{0} is not an address of this account")]
    NotOwnAddress(String),

    #[error("share {0} has no accessible membership")]
    NotAMember(ShareId),

    #[error("album has photos missing from the timeline")]
    AlbumOutsideTimeline(Vec<LinkId>),

    #[error("device {0} is not registered")]
    UnknownDevice(DeviceId),

    #[error("link {0} is not a Proton document")]
    NotADocument(LinkId),

    #[error("invalid public link \"{0}\"")]
    NotAPublicLink(String),

    #[error("uploaded content does not match expected SHA-1 {expected}")]
    ChecksumMismatch { expected: String, computed: String },

    #[error("block {index} fails verification")]
    BlockVerification { index: u32 },

    #[error("expiration is in the past")]
    ExpirationInPast,
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::Io(error)
    }
}

impl Error {
    pub(crate) fn check_signature(
        field: &'static str,
        status: SignatureStatus,
    ) -> Result<SignatureStatus> {
        match &status {
            SignatureStatus::Valid { .. }
            | SignatureStatus::Missing
            | SignatureStatus::UnknownKey { .. } => Ok(status),
            _ => Err(Error::Signature {
                field,
                status: Box::new(status),
            }),
        }
    }
}

impl From<rikka_net::Error> for Error {
    fn from(error: rikka_net::Error) -> Error {
        Error::Transfer(Box::new(error))
    }
}

impl Error {
    #[must_use]
    pub fn api(&self) -> Option<&rikka_proton_api::Error> {
        match self {
            Error::Api(error) => Some(error),
            Error::Account(error) => error.api(),
            Error::Transfer(cause) => cause.downcast_ref(),
            _ => None,
        }
    }
}
