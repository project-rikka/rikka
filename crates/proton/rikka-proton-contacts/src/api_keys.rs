use rikka_proton_account::{PublicKeys, PublishedKey, PublishedKeys};
use rikka_proton_pgp::PublicKey;

use crate::canonical::CanonicalScheme;
use crate::kt::{KEY_VERIFICATION_ERROR, KtActivation, KtStatus, KtVerdicts, KtVerification};

const NOT_COMPROMISED: i32 = 1;
const NOT_OBSOLETE: i32 = 2;
const NO_MAIL_ENCRYPTION: i32 = 4;
const PROTON_SOURCE: i32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Recipient {
    Internal,
    External,
}

#[derive(Debug, Clone)]
pub struct ApiKey {
    key: PublicKey,
    flags: i32,
    primary: bool,
    source: i32,
}

impl ApiKey {
    #[must_use]
    pub fn new(key: PublicKey, flags: i32) -> ApiKey {
        ApiKey {
            key,
            flags,
            primary: false,
            source: PROTON_SOURCE,
        }
    }

    #[must_use]
    pub fn primary(mut self, primary: bool) -> ApiKey {
        self.primary = primary;
        self
    }

    #[must_use]
    pub fn with_source(mut self, source: i32) -> ApiKey {
        self.source = source;
        self
    }

    fn from_published(published: &PublishedKey) -> Option<ApiKey> {
        let key = PublicKey::import(published.armored()).ok()?;
        Some(
            ApiKey::new(key, published.flags())
                .primary(published.is_primary())
                .with_source(published.source().unwrap_or(PROTON_SOURCE)),
        )
    }

    #[must_use]
    pub fn key(&self) -> &PublicKey {
        &self.key
    }

    #[must_use]
    pub fn flags(&self) -> i32 {
        self.flags
    }

    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.primary
    }

    #[must_use]
    pub fn source(&self) -> i32 {
        self.source
    }

    #[must_use]
    pub fn is_compromised(&self) -> bool {
        self.flags & NOT_COMPROMISED == 0
    }

    #[must_use]
    pub fn is_obsolete(&self) -> bool {
        self.flags & NOT_OBSOLETE == 0
    }

    #[must_use]
    pub fn supports_mail(&self) -> bool {
        self.flags & NO_MAIL_ENCRYPTION == 0
    }
}

#[derive(Debug, Clone)]
pub struct ApiKeys {
    keys: Vec<ApiKey>,
    recipient: Recipient,
    internal_with_disabled_e2ee: bool,
    catch_all: bool,
    warnings: Vec<String>,
    errors: Vec<String>,
    kt: Option<KtVerification>,
}

impl ApiKeys {
    #[must_use]
    pub fn new(recipient: Recipient, keys: Vec<ApiKey>) -> ApiKeys {
        ApiKeys {
            keys,
            recipient,
            internal_with_disabled_e2ee: false,
            catch_all: false,
            warnings: Vec::new(),
            errors: Vec::new(),
            kt: None,
        }
    }

    #[must_use]
    pub fn own_address() -> ApiKeys {
        ApiKeys::new(Recipient::Internal, Vec::new()).with_kt(Some(KtVerification::verified()))
    }

    #[must_use]
    pub fn with_errors(mut self, errors: Vec<String>) -> ApiKeys {
        self.errors = errors;
        self
    }

    #[must_use]
    pub fn with_kt(mut self, kt: Option<KtVerification>) -> ApiKeys {
        self.kt = kt;
        self
    }

    #[must_use]
    pub fn from_published(
        published: &PublicKeys,
        verdicts: Option<KtVerdicts<'_>>,
        activation: KtActivation,
    ) -> ApiKeys {
        let verdicts = verdicts.filter(|_| activation != KtActivation::Disabled);
        let classified = classify(published, verdicts).with_warnings(published.warnings().to_vec());
        match (activation, classified.kt) {
            (KtActivation::Disabled | KtActivation::LogOnly, _) => classified.with_kt(None),
            (KtActivation::ShowUi, Some(kt)) if kt.is_failed() => ApiKeys {
                keys: Vec::new(),
                errors: vec![KEY_VERIFICATION_ERROR.to_owned()],
                ..classified
            },
            (KtActivation::ShowUi, _) => classified,
        }
    }

    #[must_use]
    pub fn failed_verification(self, activation: KtActivation) -> ApiKeys {
        match activation {
            KtActivation::ShowUi => ApiKeys {
                keys: Vec::new(),
                errors: vec![KEY_VERIFICATION_ERROR.to_owned()],
                kt: Some(KtVerification {
                    status: KtStatus::Failed,
                    keys_changed_recently: false,
                }),
                ..self
            },
            KtActivation::LogOnly | KtActivation::Disabled => self.with_kt(None),
        }
    }

    fn with_warnings(mut self, warnings: Vec<String>) -> ApiKeys {
        self.warnings = warnings;
        self
    }

    #[must_use]
    pub fn keys(&self) -> &[ApiKey] {
        &self.keys
    }

    #[must_use]
    pub fn recipient(&self) -> Recipient {
        self.recipient
    }

    #[must_use]
    pub fn is_internal(&self) -> bool {
        self.recipient == Recipient::Internal
    }

    pub(crate) fn canonical_scheme(&self) -> CanonicalScheme {
        match self.recipient {
            Recipient::Internal => CanonicalScheme::Proton,
            Recipient::External => CanonicalScheme::Default,
        }
    }

    #[must_use]
    pub fn is_internal_with_disabled_e2ee(&self) -> bool {
        self.internal_with_disabled_e2ee
    }

    #[must_use]
    pub fn is_catch_all(&self) -> bool {
        self.catch_all
    }

    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    #[must_use]
    pub fn kt(&self) -> Option<KtVerification> {
        self.kt
    }

    #[must_use]
    pub fn is_kt_failure(&self) -> bool {
        self.errors
            .iter()
            .any(|error| error == KEY_VERIFICATION_ERROR)
    }
}

fn import_keys(keys: Option<&PublishedKeys>) -> Vec<ApiKey> {
    keys.map(|keys| {
        keys.keys()
            .iter()
            .filter_map(ApiKey::from_published)
            .collect()
    })
    .unwrap_or_default()
}

fn classify(published: &PublicKeys, verdicts: Option<KtVerdicts<'_>>) -> ApiKeys {
    let address_kt = verdicts.map(|verdicts| KtVerification::from(verdicts.address));
    let combined_kt = verdicts
        .map(|verdicts| KtVerification::combine(Some(verdicts.address), verdicts.catch_all));

    let address_keys = import_keys(Some(published.address()));
    if !address_keys.is_empty() {
        let mail_keys: Vec<ApiKey> = address_keys
            .into_iter()
            .filter(ApiKey::supports_mail)
            .collect();
        if !mail_keys.is_empty() {
            return ApiKeys::new(Recipient::Internal, mail_keys).with_kt(address_kt);
        }
        if published.has_proton_mx() {
            let mut external =
                ApiKeys::new(Recipient::External, Vec::new()).with_kt(address_kt.map(|kt| {
                    KtVerification {
                        status: if kt.is_failed() {
                            KtStatus::Failed
                        } else {
                            KtStatus::Unverified
                        },
                        keys_changed_recently: false,
                    }
                }));
            external.internal_with_disabled_e2ee = true;
            return external;
        }
    }

    let unverified = import_keys(published.unverified());
    let internal_unverified: Vec<ApiKey> = unverified
        .iter()
        .filter(|key| key.source == PROTON_SOURCE && key.supports_mail())
        .cloned()
        .collect();
    if !internal_unverified.is_empty() {
        return ApiKeys::new(Recipient::Internal, internal_unverified).with_kt(combined_kt);
    }

    let catch_all: Vec<ApiKey> = import_keys(published.catch_all())
        .into_iter()
        .filter(ApiKey::supports_mail)
        .collect();
    if !catch_all.is_empty() {
        let catch_all_kt = verdicts
            .and_then(|verdicts| verdicts.catch_all)
            .map(|verdict| {
                let mut kt = KtVerification::from(verdict);
                if combined_kt.is_some_and(|combined| combined.is_failed()) {
                    kt.status = KtStatus::Failed;
                }
                kt.keys_changed_recently =
                    combined_kt.is_some_and(|combined| combined.keys_changed_recently);
                kt
            });
        let mut internal = ApiKeys::new(Recipient::Internal, catch_all).with_kt(catch_all_kt);
        internal.catch_all = true;
        return internal;
    }

    let external = unverified
        .into_iter()
        .find(|key| key.source != PROTON_SOURCE && key.supports_mail());
    ApiKeys::new(Recipient::External, external.into_iter().collect()).with_kt(combined_kt)
}
