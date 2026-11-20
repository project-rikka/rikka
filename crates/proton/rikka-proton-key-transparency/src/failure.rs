use rikka_proton_account::{KeyDifference, KeyList, KeyListMismatch};
use serde::{Deserialize, Serialize};

use crate::proof::ProofKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[non_exhaustive]
pub enum Failure {
    #[error("VRF public key is not an edwards25519 point")]
    InvalidVrfKey,
    #[error("invalid VRF proof")]
    VrfProof,
    #[error("proof has {found} neighbors, expected 256")]
    NeighborCount { found: usize },
    #[error("proof neighbor is not a 32-byte hex hash")]
    MalformedNeighbor,
    #[error("proof does not match epoch tree hash")]
    TreeHashMismatch,
    #[error("absence proof has non-empty revision subtree")]
    RevisionSubtreeNotEmpty,
    #[error("expected {expected} proof, found {found}")]
    UnexpectedProof {
        expected: ProofKind,
        found: ProofKind,
    },
    #[error("signed key list is missing data, minimum epoch or revision")]
    IncompleteSignedKeyList,
    #[error("obsolescence token is not lowercase hex")]
    MalformedObsolescenceToken,
    #[error("invalid signed key list data")]
    MalformedKeyList,
    #[error("key list is empty")]
    NoKeys,
    #[error("{listed} keys listed, signed key list has {signed}")]
    KeyCountMismatch { listed: usize, signed: usize },
    #[error("key {fingerprint} differs from signed key list in {difference}")]
    KeyMismatch {
        fingerprint: String,
        difference: KeyDifference,
    },
    #[error("signed key list has revision 0")]
    RevisionZero,
    #[error("signed key list has no minimum epoch")]
    MissingMinEpoch,
    #[error("signed key list has neither data nor obsolescence token")]
    MissingContent,
    #[error("failed to parse epoch certificate: {0}")]
    MalformedCertificate(String),
    #[error("epoch certificate issuer {0} is not pinned")]
    UnknownIssuer(i32),
    #[error("epoch certificate does not chain to pinned roots: {0}")]
    CertificateChain(String),
    #[error("epoch certificate has no valid SCT from a known log operator")]
    InsufficientTimestamps,
    #[error("epoch hashes are not 32-byte hex hashes")]
    MalformedEpoch,
    #[error("chain hash does not match previous chain hash and tree hash")]
    ChainHashMismatch,
    #[error("epoch certificate names {found:?}, expected {expected}")]
    AlternativeNameMismatch {
        expected: String,
        found: Option<String>,
    },
    #[error("latest epoch certificate is older than 72 hours")]
    StaleEpoch,
    #[error("epoch is older than 90 days")]
    ExpiredEpoch,
    #[error("signed key list has no revision")]
    MissingRevision,
    #[error("signed key list minimum epochs do not increase")]
    MinEpochsNotIncreasing,
    #[error("signed key list signature times do not increase")]
    SignatureTimesNotIncreasing,
    #[error("audited signed key list differs from current one")]
    AuditedKeyListMismatch,
    #[error("invalid signed key list signature")]
    UnverifiedSignature,
    #[error("address without keys has no obsolescence token")]
    ExpectedObsolescence,
    #[error("pending signed key list has no expected minimum epoch")]
    MissingExpectedMinEpoch,
    #[error("revision {revision} not included within allowed interval")]
    PendingKeyListIgnored { revision: u32 },
    #[error("pending revision {revision} is no longer served")]
    PendingKeyListMissing { revision: u32 },
    #[error("pending revision {revision} served with different content")]
    PendingKeyListChanged { revision: u32 },
    #[error("published signed key list not found in address revisions")]
    PublishedKeyListNotFound,
    #[error("address has no primary key")]
    NoPrimaryKey,
    #[error("only v4 keys require resigning")]
    UnexpectedKeyVersion,
    #[error("key is not in signed key list")]
    KeyNotListed,
    #[error("key is marked compromised in signed key list")]
    KeyCompromised,
}

impl From<KeyListMismatch> for Failure {
    fn from(mismatch: KeyListMismatch) -> Failure {
        match mismatch {
            KeyListMismatch::NoKeys => Failure::NoKeys,
            KeyListMismatch::Count { listed, signed } => {
                Failure::KeyCountMismatch { listed, signed }
            }
            KeyListMismatch::Key {
                fingerprint,
                difference,
            } => Failure::KeyMismatch {
                fingerprint,
                difference,
            },
        }
    }
}

pub(crate) fn parse_key_list(data: &str) -> Result<KeyList, Failure> {
    KeyList::parse(data).map_err(|_| Failure::MalformedKeyList)
}
