mod clock;
mod google;
mod hotp;
mod model;
mod otp;
mod proto;
mod secret;
mod steam;
mod totp;
mod uri;

pub use google::{DigitCount, MigrationAlgorithm, MigrationPayload, OtpParameters, OtpType};
pub use hotp::Hotp;
pub use model::Entry;
pub use otp::Otp;
pub use proto::{
    AuthenticatorEntry, AuthenticatorEntryContent, AuthenticatorEntryContentSteam,
    AuthenticatorEntryContentTotp, AuthenticatorEntryMetadata, EntryContent,
};
pub use steam::Steam;
pub use totp::Totp;

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}
