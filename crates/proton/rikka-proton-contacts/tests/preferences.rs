use rikka_proton_account::PublicKeys;
use rikka_proton_contacts::{
    ApiKey, ApiKeys, CanonicalScheme, EmailPreferences, KEY_VERIFICATION_ERROR, KtActivation,
    KtAvailability, KtStatus, KtVerdicts, KtVerification, MailDefaults, MessageIntent, MimeType,
    PackageScheme, PgpScheme, PinnedKeys, PreferenceError, Property, Recipient, SelfSend, VCard,
    VerificationKeys, canonical_email, canonical_email_by_guess, canonical_internal_email,
    email_mismatch, encryption_preferences,
};
use rikka_proton_key_transparency::{Failure, Pending, Unverifiable, Verdict};
use rikka_proton_pgp::{Pgp, PublicKey};
use serde_json::json;

use rikka_test_proton::new_key;

const VALID: i32 = 3;
const COMPROMISED: i32 = 2;

fn card() -> VCard {
    VCard::new()
        .with(Property::text("FN", "Bob"))
        .with(Property::text("EMAIL", "bob@example.com").with_group("item1"))
}

#[test]
fn email_preferences_round_trip() {
    let pgp = Pgp::new();
    let pinned = new_key(&pgp, "bob@example.com").public_key();
    let preferences = EmailPreferences {
        pinned_keys: vec![pinned.clone()],
        encrypt_to_pinned: Some(true),
        encrypt_to_untrusted: None,
        sign: Some(false),
        scheme: Some(PgpScheme::Inline),
        mime_type: Some(MimeType::PlainText),
    };
    let mut card = card();
    preferences.apply_to(&mut card, "bob@example.com").unwrap();

    let serialized = card.to_string();
    assert!(serialized.contains("item1.KEY;PREF=1:data:application/pgp-keys;base64,"));
    assert!(serialized.contains("item1.X-PM-ENCRYPT:true"));
    assert!(serialized.contains("item1.X-PM-SCHEME:pgp-inline"));

    let reread =
        EmailPreferences::from_card(&VCard::parse(&serialized).unwrap(), "BOB@example.com")
            .unwrap();
    assert_eq!(
        reread
            .pinned_keys
            .iter()
            .map(PublicKey::fingerprint)
            .collect::<Vec<_>>(),
        vec![pinned.fingerprint()]
    );
    assert_eq!(reread.encrypt_to_pinned, Some(true));
    assert_eq!(reread.sign, Some(false));
    assert_eq!(reread.scheme, Some(PgpScheme::Inline));
    assert_eq!(reread.mime_type, Some(MimeType::PlainText));

    assert!(
        EmailPreferences::default()
            .apply_to(&mut VCard::new(), "nobody@example.com")
            .is_err()
    );
}

struct Case {
    name: &'static str,
    api: ApiKeys,
    pinned: PinnedKeys,
    intent: MessageIntent,
    scheme: PackageScheme,
    mime_type: MimeType,
    encrypt: bool,
    key_pinned: bool,
    error: Option<PreferenceError>,
}

fn pinned(
    keys: Vec<PublicKey>,
    encrypt: Option<bool>,
    sign: Option<bool>,
    scheme: Option<PgpScheme>,
) -> PinnedKeys {
    PinnedKeys::for_contact(EmailPreferences {
        pinned_keys: keys,
        encrypt_to_pinned: encrypt,
        sign,
        scheme,
        ..EmailPreferences::default()
    })
    .with_verification(true, None)
}

#[test]
fn send_preferences_decision_table() {
    let pgp = Pgp::new();
    let proton = new_key(&pgp, "bob@proton.test").public_key();
    let other = new_key(&pgp, "bob@proton.test").public_key();
    let outside = new_key(&pgp, "bob@example.com").public_key();
    let internal = |flags: i32| {
        ApiKeys::new(
            Recipient::Internal,
            vec![ApiKey::new(proton.clone(), flags)],
        )
    };
    let external = ApiKeys::new(Recipient::External, Vec::new());

    let cases = [
        Case {
            name: "internal without a contact",
            api: internal(VALID),
            pinned: PinnedKeys::none(),
            intent: MessageIntent::default(),
            scheme: PackageScheme::Internal,
            mime_type: MimeType::Html,
            encrypt: true,
            key_pinned: false,
            error: None,
        },
        Case {
            name: "internal with the primary key pinned",
            api: internal(VALID),
            pinned: pinned(vec![proton.clone()], None, None, None),
            intent: MessageIntent::default(),
            scheme: PackageScheme::Internal,
            mime_type: MimeType::Html,
            encrypt: true,
            key_pinned: true,
            error: None,
        },
        Case {
            name: "internal with another key pinned",
            api: internal(VALID),
            pinned: pinned(vec![other.clone()], None, None, None),
            intent: MessageIntent::default(),
            scheme: PackageScheme::Internal,
            mime_type: MimeType::Html,
            encrypt: true,
            key_pinned: false,
            error: Some(PreferenceError::PrimaryNotPinned),
        },
        Case {
            name: "internal with a compromised key",
            api: internal(COMPROMISED),
            pinned: PinnedKeys::none(),
            intent: MessageIntent::default(),
            scheme: PackageScheme::Internal,
            mime_type: MimeType::Html,
            encrypt: true,
            key_pinned: false,
            error: Some(PreferenceError::PrimaryCannotSend),
        },
        Case {
            name: "external without keys",
            api: external.clone(),
            pinned: PinnedKeys::none(),
            intent: MessageIntent::default(),
            scheme: PackageScheme::Clear,
            mime_type: MimeType::Html,
            encrypt: false,
            key_pinned: false,
            error: None,
        },
        Case {
            name: "external pinned inline",
            api: external.clone(),
            pinned: pinned(
                vec![outside.clone()],
                Some(true),
                None,
                Some(PgpScheme::Inline),
            ),
            intent: MessageIntent::default(),
            scheme: PackageScheme::PgpInline,
            mime_type: MimeType::PlainText,
            encrypt: true,
            key_pinned: true,
            error: None,
        },
        Case {
            name: "external pinned but only signing",
            api: external.clone(),
            pinned: pinned(vec![outside.clone()], Some(false), Some(true), None),
            intent: MessageIntent::default(),
            scheme: PackageScheme::PgpMime,
            mime_type: MimeType::Multipart,
            encrypt: false,
            key_pinned: false,
            error: None,
        },
        Case {
            name: "external with an unverified contact",
            api: external.clone(),
            pinned: PinnedKeys::for_contact(EmailPreferences::default())
                .with_verification(false, None),
            intent: MessageIntent::default(),
            scheme: PackageScheme::Clear,
            mime_type: MimeType::Html,
            encrypt: false,
            key_pinned: false,
            error: Some(PreferenceError::ContactSignatureNotVerified),
        },
        Case {
            name: "external sent encrypted to outside",
            api: external,
            pinned: PinnedKeys::none(),
            intent: MessageIntent {
                encrypted_outside: true,
                sign: false,
                mime_type: Some(MimeType::PlainText),
            },
            scheme: PackageScheme::EncryptedOutside,
            mime_type: MimeType::PlainText,
            encrypt: true,
            key_pinned: false,
            error: None,
        },
    ];

    for case in cases {
        let preferences = encryption_preferences(
            "bob@proton.test",
            &case.api,
            &case.pinned,
            &MailDefaults::default(),
            None,
            pgp.now(),
        );
        let send = preferences.send_preferences(&case.intent);
        assert_eq!(send.scheme, case.scheme, "{}", case.name);
        assert_eq!(send.mime_type, case.mime_type, "{}", case.name);
        assert_eq!(send.encrypt, case.encrypt, "{}", case.name);
        assert_eq!(send.key_pinned, case.key_pinned, "{}", case.name);
        assert_eq!(send.error, case.error, "{}", case.name);
    }
}

#[test]
fn self_send_uses_own_key() {
    let pgp = Pgp::new();
    let key = new_key(&pgp, "me@proton.test").public_key();
    let own = SelfSend {
        can_receive: true,
        has_keys: true,
        key: Some(key.clone()),
        can_send: true,
    };
    let preferences = encryption_preferences(
        "me@proton.test",
        &ApiKeys::own_address(),
        &PinnedKeys::none(),
        &MailDefaults::default(),
        Some(&own),
        pgp.now(),
    );
    let send = preferences.send_preferences(&MessageIntent::default());

    assert_eq!(send.scheme, PackageScheme::Internal);
    assert!(send.encrypt && send.sign);
    assert_eq!(
        send.key.map(|key| key.fingerprint()),
        Some(key.fingerprint())
    );

    let disabled = SelfSend {
        can_receive: false,
        ..own
    };
    let refused = encryption_preferences(
        "me@proton.test",
        &ApiKeys::own_address(),
        &PinnedKeys::none(),
        &MailDefaults::default(),
        Some(&disabled),
        pgp.now(),
    );
    assert_eq!(refused.error, Some(PreferenceError::InternalUserDisabled));
}

fn published(keys: &[(&PublicKey, i32, bool)], proton_mx: bool) -> PublicKeys {
    let keys: Vec<serde_json::Value> = keys
        .iter()
        .map(|(key, flags, primary)| {
            json!({
                "Flags": flags,
                "Primary": u8::from(*primary),
                "PublicKey": key.export_armored().unwrap(),
                "Source": 0
            })
        })
        .collect();
    serde_json::from_value(
        json!({"Address": {"Keys": keys}, "ProtonMX": proton_mx, "Warnings": []}),
    )
    .unwrap()
}

#[test]
fn published_keys_set_recipient_kind() {
    let pgp = Pgp::new();
    let key = new_key(&pgp, "bob@proton.test").public_key();

    let internal = ApiKeys::from_published(
        &published(&[(&key, VALID, true)], true),
        None,
        KtActivation::ShowUi,
    );
    assert_eq!(internal.recipient(), Recipient::Internal);
    assert_eq!(internal.keys().len(), 1);
    assert!(internal.keys()[0].is_primary());
    assert!(internal.kt().is_none());

    let forwarding = ApiKeys::from_published(
        &published(&[(&key, VALID | 4, true)], true),
        None,
        KtActivation::ShowUi,
    );
    assert_eq!(forwarding.recipient(), Recipient::External);
    assert!(forwarding.is_internal_with_disabled_e2ee());
    let send = encryption_preferences(
        "bob@proton.test",
        &forwarding,
        &PinnedKeys::none(),
        &MailDefaults::default(),
        None,
        pgp.now(),
    )
    .send_preferences(&MessageIntent::default());
    assert!(!send.encrypt);
    assert!(send.encryption_disabled);
    assert_eq!(send.scheme, PackageScheme::Clear);

    let unknown = ApiKeys::from_published(&PublicKeys::default(), None, KtActivation::ShowUi);
    assert_eq!(unknown.recipient(), Recipient::External);
    assert!(unknown.keys().is_empty());
}

#[test]
fn flagged_primary_key_wins() {
    let pgp = Pgp::new();
    let older = new_key(&pgp, "bob@proton.test").public_key();
    let primary = new_key(&pgp, "bob@proton.test").public_key();
    let api = ApiKeys::from_published(
        &published(&[(&older, VALID, false), (&primary, VALID, true)], true),
        None,
        KtActivation::Disabled,
    );
    let preferences = encryption_preferences(
        "bob@proton.test",
        &api,
        &PinnedKeys::none(),
        &MailDefaults::default(),
        None,
        pgp.now(),
    );

    assert_eq!(
        preferences.send_key.map(|key| key.fingerprint()),
        Some(primary.fingerprint())
    );
    assert_eq!(preferences.api_keys[0].fingerprint(), primary.fingerprint());
}

#[test]
fn kt_verdicts_reach_send_preferences() {
    let pgp = Pgp::new();
    let key = new_key(&pgp, "bob@proton.test").public_key();
    let keys = published(&[(&key, VALID, true)], true);
    let failed = Verdict::Failed(Failure::RevisionZero);
    let cases = [
        (
            "verified",
            Verdict::Verified,
            KtActivation::ShowUi,
            Some((KtStatus::Verified, false)),
            None,
        ),
        (
            "recently changed",
            Verdict::Pending(Pending::Keys),
            KtActivation::ShowUi,
            Some((KtStatus::Verified, true)),
            None,
        ),
        (
            "obsolete",
            Verdict::Pending(Pending::Obsolescence),
            KtActivation::ShowUi,
            Some((KtStatus::Unverified, true)),
            None,
        ),
        (
            "absent",
            Verdict::Unverifiable(Unverifiable::Absent),
            KtActivation::ShowUi,
            Some((KtStatus::Unverified, false)),
            None,
        ),
        (
            "failed and shown",
            failed.clone(),
            KtActivation::ShowUi,
            Some((KtStatus::Failed, false)),
            Some(PreferenceError::KeyTransparencyFailed),
        ),
        (
            "failed but only logged",
            failed,
            KtActivation::LogOnly,
            None,
            None,
        ),
        (
            "disabled",
            Verdict::Verified,
            KtActivation::Disabled,
            None,
            None,
        ),
    ];

    for (name, verdict, activation, expected, error) in cases {
        let verdicts = KtVerdicts {
            address: &verdict,
            catch_all: None,
        };
        let api = ApiKeys::from_published(&keys, Some(verdicts), activation);
        let send = encryption_preferences(
            "bob@proton.test",
            &api,
            &PinnedKeys::none(),
            &MailDefaults::default(),
            None,
            pgp.now(),
        )
        .send_preferences(&MessageIntent::default());
        assert_eq!(
            send.kt.map(|kt| (kt.status, kt.keys_changed_recently)),
            expected,
            "{name}"
        );
        assert_eq!(send.error, error, "{name}");
        if error.is_some() {
            assert!(api.keys().is_empty(), "{name}");
            assert_eq!(api.errors(), [KEY_VERIFICATION_ERROR], "{name}");
        }
    }

    let broken = ApiKeys::from_published(&keys, None, KtActivation::ShowUi)
        .failed_verification(KtActivation::ShowUi);
    assert_eq!(broken.kt().map(|kt| kt.status), Some(KtStatus::Failed));
    assert!(broken.is_kt_failure());
}

#[test]
fn canonical_emails_follow_schemes() {
    let cases = [
        (
            "Jo.Hn-D_oe+news@Proton.me",
            "johndoe@proton.me",
            "johndoe@proton.me",
        ),
        ("J.Doe+tag@GMail.com", "jdoe@gmail.com", "jdoe@gmail.com"),
        (
            "First.Last+x@Outlook.com",
            "first.last@outlook.com",
            "firstlast@outlook.com",
        ),
        (
            "Mixed.Case+Plus@Example.org",
            "mixed.case+plus@example.org",
            "mixedcase@example.org",
        ),
    ];
    for (email, guessed, internal) in cases {
        assert_eq!(canonical_email_by_guess(email), guessed, "{email}");
        assert_eq!(canonical_internal_email(email), internal, "{email}");
    }
    assert_eq!(
        canonical_email("NoDomain+x", CanonicalScheme::Plus),
        "nodomain"
    );
}

#[test]
fn key_without_address_warns() {
    let pgp = Pgp::new();
    let key = new_key(&pgp, "bob@proton.me").public_key();

    assert!(email_mismatch(&key, "B.O.B+work@proton.me", Recipient::Internal).is_empty());
    let warned = email_mismatch(&key, "b.o.b@proton.me", Recipient::External);
    assert!(warned.is_empty());
    let other = email_mismatch(&key, "alice@proton.me", Recipient::Internal);
    assert_eq!(
        other,
        vec![
            "Email address not found among user ids defined in sending key (bob@proton.me)"
                .to_owned()
        ]
    );

    let api = ApiKeys::new(
        Recipient::Internal,
        vec![ApiKey::new(key, VALID).primary(true)],
    );
    let preferences = encryption_preferences(
        "alice@proton.me",
        &api,
        &PinnedKeys::none(),
        &MailDefaults::default(),
        None,
        pgp.now(),
    );
    assert_eq!(
        preferences
            .send_preferences(&MessageIntent::default())
            .warnings,
        other
    );
}

#[test]
fn verification_prefers_pinned_keys() {
    let pgp = Pgp::new();
    let api_key = new_key(&pgp, "bob@proton.test").public_key();
    let pin = new_key(&pgp, "bob@proton.test").public_key();
    let internal = |kt: Option<KtVerification>| {
        ApiKeys::new(
            Recipient::Internal,
            vec![ApiKey::new(api_key.clone(), VALID).primary(true)],
        )
        .with_kt(kt)
    };
    let verified = KtVerification::verified();
    let unverified = KtVerification {
        status: KtStatus::Unverified,
        keys_changed_recently: false,
    };
    let pinned = PinnedKeys::for_contact(EmailPreferences {
        pinned_keys: vec![pin.clone()],
        ..EmailPreferences::default()
    })
    .with_verification(true, None);
    let fingerprints = |keys: &VerificationKeys| {
        keys.keys
            .iter()
            .map(PublicKey::fingerprint)
            .collect::<Vec<_>>()
    };

    let cases = [
        (
            "pinned",
            internal(Some(verified)),
            pinned.clone(),
            KtAvailability::Enabled,
            vec![pin.fingerprint()],
        ),
        (
            "kt verified",
            internal(Some(verified)),
            PinnedKeys::none(),
            KtAvailability::Enabled,
            vec![api_key.fingerprint()],
        ),
        (
            "kt unverified",
            internal(Some(unverified)),
            PinnedKeys::none(),
            KtAvailability::Enabled,
            Vec::new(),
        ),
        (
            "without kt",
            internal(None),
            PinnedKeys::none(),
            KtAvailability::Disabled,
            vec![api_key.fingerprint()],
        ),
        (
            "external",
            ApiKeys::new(
                Recipient::External,
                vec![ApiKey::new(api_key.clone(), VALID)],
            ),
            PinnedKeys::none(),
            KtAvailability::Disabled,
            Vec::new(),
        ),
    ];
    for (name, api, pinned, kt, expected) in cases {
        assert_eq!(
            fingerprints(&VerificationKeys::select(&api, &pinned, kt)),
            expected,
            "{name}"
        );
    }
}
