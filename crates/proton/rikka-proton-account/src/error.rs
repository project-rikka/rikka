use rikka_proton_pgp::SignatureStatus;

use crate::auth_device::{AuthDeviceError, AuthDeviceId};
use crate::identifier::{AddressId, KeyId};
use crate::signed_key_list::SignedKeyListCheck;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Api(#[from] rikka_proton_api::Error),

    #[error(transparent)]
    Pgp(#[from] rikka_proton_pgp::Error),

    #[error("username is empty")]
    MissingUsername,

    #[error("API response has no {0}")]
    Incomplete(&'static str),

    #[error("incorrect password")]
    WrongPassword,

    #[error("account requires SSO login")]
    IdentityProviderRequired,

    #[error("account does not use SSO")]
    PasswordRequired,

    #[error("second factor required")]
    SecondFactorRequired,

    #[error("mailbox password required")]
    MailboxPasswordRequired,

    #[error("account has no primary key")]
    NoPrimaryKey,

    #[error("no key salt for key {0}")]
    NoKeySalt(KeyId),

    #[error("key salt is not valid base64")]
    MalformedSalt,

    #[error("password unlocks no account key")]
    NoKeyUnlocked,

    #[error("account has no unlocked user key")]
    NoUserKey,

    #[error("key token is not 64 hex characters")]
    MalformedKeyToken,

    #[error("invalid key token signature: {0}")]
    UnsignedKeyToken(Box<SignatureStatus>),

    #[error("address key {0} has no token")]
    NoKeyToken(KeyId),

    #[error("session fork key must be 32 bytes")]
    ForkKeyLength,

    #[error("fork key is not valid base64")]
    MalformedForkKey,

    #[error("forked session has no payload")]
    NoForkPayload,

    #[error("malformed forked session payload")]
    MalformedForkPayload,

    #[error("failed to decrypt forked session payload")]
    ForkPayloadDecryption,

    #[error("invalid forked session payload JSON")]
    InvalidForkPayloadJson(#[source] serde_json::Error),

    #[error("account already has keys")]
    KeysAlreadySetUp,

    #[error("keys were unlocked without a passphrase")]
    NoPassphrase,

    #[error("address {0} has no unlocked primary key")]
    NoPrimaryAddressKey(AddressId),

    #[error("account has no address")]
    NoAddress,

    #[error("address {0} is not one of the account's addresses")]
    UnknownAddress(AddressId),

    #[error("address {0} has no unlocked key")]
    NoAddressKey(AddressId),

    #[error("key {0} not found in required state")]
    NoSuchKey(KeyId),

    #[error("key {0} is already active")]
    KeyAlreadyActive(KeyId),

    #[error("old password unlocks neither key {0} nor its token")]
    KeyNotReactivated(KeyId),

    #[error("untrusted signed key list for {email}: {check}")]
    UntrustedSignedKeyList {
        email: String,
        check: Box<SignedKeyListCheck>,
    },

    #[error("invalid activation token signature on address key {key}: {status}")]
    UnsignedActivation {
        key: KeyId,
        status: Box<SignatureStatus>,
    },

    #[error("device {device}: {reason}")]
    AuthDevice {
        device: AuthDeviceId,
        reason: AuthDeviceError,
    },

    #[error("device secret must be 32 base64-encoded bytes")]
    MalformedDeviceSecret,

    #[error("failed to decrypt device secret")]
    DeviceSecretDecryption,

    #[error("device {0} has no activation token")]
    NoActivationToken(AuthDeviceId),

    #[error("confirmation code does not match device {0}")]
    WrongConfirmationCode(AuthDeviceId),

    #[error("device {device} uses unknown confirmation code version {version}")]
    UnknownConfirmationCodeVersion { device: AuthDeviceId, version: u8 },

    #[error("invalid signed key list data")]
    MalformedKeyList,

    #[error("key list has no v4 primary key")]
    KeyListWithoutPrimary,

    #[error("cannot delete primary address key {0}")]
    PrimaryKeyDeletion(KeyId),

    #[error("post-quantum address keys require token-based address keys")]
    PostQuantumNeedsTokens,

    #[error("organization private key unavailable")]
    NoOrganizationKey,

    #[error("invalid organization signature on member key token {key}: {status}")]
    UnsignedMemberToken {
        key: KeyId,
        status: Box<SignatureStatus>,
    },

    #[error("untrusted organization invitation: {0}")]
    UntrustedOrganization(Box<SignatureStatus>),

    #[error("invalid unprivatization invitation JSON")]
    MalformedInvitation,

    #[error("no address {0} to accept invitation")]
    NoInvitationAddress(String),

    #[error("security key name is {length} characters, limit is {limit}")]
    SecurityKeyNameTooLong { length: usize, limit: usize },
}

impl Error {
    #[must_use]
    pub fn api(&self) -> Option<&rikka_proton_api::Error> {
        match self {
            Error::Api(error) => Some(error),
            _ => None,
        }
    }
}
