mod api_keys;
mod cached;
mod canonical;
mod card;
mod client;
mod contact;
mod contact_email;
mod creation;
mod decrypted_contact;
mod email_preferences;
mod encryption_preferences;
mod error;
mod event;
mod feed;
mod group;
mod group_draft;
mod identifier;
mod import;
mod kt;
mod merging;
mod opening;
mod outcome;
mod pinned_keys;
mod preparation;
mod property;
mod queries;
mod quoted_printable;
mod scheme;
mod sealing;
mod send_preferences;
mod structured;
mod upgrade;
mod vcard;
mod verification_keys;
mod verifiers;
mod version;

pub use api_keys::{ApiKey, ApiKeys, Recipient};
pub use canonical::{
    CanonicalScheme, canonical_email, canonical_email_by_guess, canonical_internal_email,
    user_id_email,
};
pub use card::{CardType, ContactCard};
pub use client::ContactsClient;
pub use contact::Contact;
pub use contact_email::ContactEmail;
pub use creation::ContactCreation;
pub use decrypted_contact::{CardProblem, DecryptedContact};
pub use email_preferences::EmailPreferences;
pub use encryption_preferences::{
    EncryptionPreferences, MailDefaults, PreferenceError, SelfSend, email_mismatch,
    encryption_preferences,
};
pub use error::{Error, Result};
pub use event::{
    ContactBook, ContactChange, ContactEmailChange, ContactEvents, ContactModel, GroupChange,
};
pub use group::ContactGroup;
pub use group_draft::GroupDraft;
pub use identifier::{ContactEmailId, ContactId, GroupId};
pub use import::parse_vcf;
pub use kt::{
    KEY_VERIFICATION_ERROR, KtActivation, KtAvailability, KtStatus, KtVerdicts, KtVerification,
};
pub use merging::{find_duplicates, merge_vcards};
pub use opening::decrypt_cards;
pub use outcome::ContactReply;
pub use pinned_keys::PinnedKeys;
pub use preparation::{generate_uid, prepare_for_saving};
pub use property::Property;
pub use queries::{ContactEmailQuery, ContactQuery, ExportQuery};
pub use rikka_proton_account::EventId;
pub use rikka_proton_api::EventAction;
pub use rikka_proton_contentline::Parameter;
pub use scheme::{MimeType, PackageScheme, PgpScheme};
pub use sealing::{CardSplit, seal_vcard, split_vcard};
pub use send_preferences::{MessageIntent, SendPreferences};
pub use structured::{PostalAddress, StructuredName};
pub use vcard::{VCard, write_vcards};
pub use verification_keys::VerificationKeys;
pub use version::Version;
