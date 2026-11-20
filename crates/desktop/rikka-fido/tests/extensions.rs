use hmac::{Hmac, KeyInit, Mac as _};
use rikka_fido::cbor::Value;
use rikka_fido::{
    AssertionRequest, CredentialProtection, Error, Fido, LargeBlobSupport, PrfValues,
    RegistrationRequest, RelyingParty, ResidentKey, User, UserVerification,
};
use rikka_test_fido::{FakeKey, Kind};
use sha2::Sha256;

use crate::support::{CHALLENGE, ORIGIN, RP, SILENT, credential, drive, sha256};

const MAKE_CREDENTIAL: u8 = 0x01;

fn registration() -> RegistrationRequest {
    RegistrationRequest::new(
        ORIGIN,
        RelyingParty::new(RP),
        User::new(*b"user-7").with_name("person@example.com"),
        CHALLENGE,
    )
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn prf_salt(input: &[u8]) -> [u8; 32] {
    sha256([b"WebAuthn PRF\0".as_slice(), input].concat())
}

#[test]
fn reports_cred_protect_props_min_pin_length() {
    let key = FakeKey::new(Kind::Ctap21);
    let (made, _) = drive(
        Fido::with_transport(key.clone())
            .register(
                registration()
                    .cred_protect(CredentialProtection::UserVerificationRequired, true)
                    .cred_props()
                    .min_pin_length(),
            )
            .expect("valid"),
        SILENT,
    );
    let registration = made.expect("the key makes a credential");

    assert_eq!(
        registration.cred_protect(),
        Some(CredentialProtection::UserVerificationRequired)
    );
    assert_eq!(registration.cred_props(), Some(false));
    assert_eq!(registration.min_pin_length(), Some(4));
    assert_eq!(key.protection(registration.credential_id()), Some(3));

    let ctap_request = key.requests(MAKE_CREDENTIAL).pop().expect("one request");
    let extensions = ctap_request.get(0x06).expect("extensions");
    assert_eq!(extensions.get("credProtect"), Some(&Value::from(3)));
    assert_eq!(extensions.get("minPinLength"), Some(&Value::Bool(true)));

    let (signed, _) = drive(
        Fido::with_transport(key)
            .assert(
                AssertionRequest::new(ORIGIN, RP, CHALLENGE)
                    .allow(registration.credential_id())
                    .user_verification(UserVerification::Discouraged),
            )
            .expect("valid"),
        SILENT,
    );
    assert!(
        matches!(signed, Err(Error::NoCredentials)),
        "a protected credential hides without verification"
    );
}

#[test]
fn enforced_cred_protect_requires_support() {
    let cases = [
        (Kind::Ctap20, true, false),
        (Kind::Ctap20, false, true),
        (Kind::U2f, true, false),
    ];

    for (kind, enforce, succeeds) in cases {
        let (made, _) = drive(
            Fido::with_transport(FakeKey::new(kind))
                .register(
                    registration()
                        .cred_protect(CredentialProtection::UserVerificationRequired, enforce),
                )
                .expect("valid"),
            SILENT,
        );

        match made {
            Ok(registration) => {
                assert!(succeeds, "{kind:?}");
                assert_eq!(registration.cred_protect(), None);
            }
            Err(Error::ExtensionUnsupported("credProtect")) => assert!(!succeeds, "{kind:?}"),
            Err(error) => panic!("{kind:?}: {error}"),
        }
    }
}

#[test]
fn prf_and_hmac_secret_evaluate_secret() {
    for protocols in [&[2, 1][..], &[1][..]] {
        let key = FakeKey::new(Kind::Ctap21).with_protocols(protocols);
        let (made, _) = drive(
            Fido::with_transport(key.clone())
                .register(
                    registration().prf_eval(PrfValues::new(*b"first").with_second(*b"second")),
                )
                .expect("valid"),
            SILENT,
        );
        let registration = made.expect("the key makes a credential");
        let id = registration.credential_id().to_vec();
        let random = key.cred_random(&id, false).expect("the credential secret");

        let prf = registration.prf().expect("prf was asked");
        assert!(prf.enabled(), "{protocols:?}");
        let results = prf.results().expect("hmac-secret-mc answered");
        assert_eq!(results.first(), hmac(&random, &prf_salt(b"first")));
        assert_eq!(
            results.second(),
            Some(hmac(&random, &prf_salt(b"second")).as_slice())
        );

        let (signed, _) = drive(
            Fido::with_transport(key.clone())
                .assert(
                    AssertionRequest::new(ORIGIN, RP, CHALLENGE)
                        .allow(id.clone())
                        .prf(PrfValues::new(*b"login")),
                )
                .expect("valid"),
            SILENT,
        );
        let assertion = signed.expect("the key signs");
        let results = assertion.prf().expect("prf results");
        assert_eq!(
            results.first(),
            hmac(&random, &prf_salt(b"login")),
            "{protocols:?}"
        );
        assert_eq!(results.second(), None);
        assert_eq!(assertion.hmac_secret(), None);

        let (signed, _) = drive(
            Fido::with_transport(key)
                .assert(
                    AssertionRequest::new(ORIGIN, RP, CHALLENGE)
                        .allow(id)
                        .hmac_secret([0x01; 32], Some([0x02; 32])),
                )
                .expect("valid"),
            SILENT,
        );
        let assertion = signed.expect("the key signs");
        let outputs = assertion.hmac_secret().expect("hmac-secret outputs");
        assert_eq!(outputs.first(), hmac(&random, &[0x01; 32]));
        assert_eq!(
            outputs.second(),
            Some(hmac(&random, &[0x02; 32]).as_slice())
        );
        assert_eq!(assertion.prf(), None);
    }
}

#[test]
fn prf_without_hmac_secret_mc_only_enables() {
    let (made, _) = drive(
        Fido::with_transport(FakeKey::new(Kind::Ctap20))
            .register(
                registration()
                    .prf_eval(PrfValues::new(*b"first"))
                    .hmac_create_secret(),
            )
            .expect("valid"),
        SILENT,
    );
    let registration = made.expect("the key makes a credential");

    let prf = registration.prf().expect("prf was asked");
    assert!(prf.enabled());
    assert_eq!(prf.results(), None);
    assert_eq!(registration.hmac_create_secret(), Some(true));
}

#[test]
fn prf_evaluation_follows_signing_credential() {
    let stored_credential = credential(0x21, 32);
    let other = credential(0x81, 32);
    let key = FakeKey::new(Kind::Ctap21).with_credential(RP, stored_credential.clone());
    let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE)
        .allow(other.clone())
        .allow(stored_credential.clone())
        .prf(PrfValues::new(*b"default"))
        .prf_for(other, PrfValues::new(*b"other"))
        .prf_for(stored_credential.clone(), PrfValues::new(*b"held"));

    let (signed, _) = drive(
        Fido::with_transport(key.clone())
            .assert(request)
            .expect("valid"),
        SILENT,
    );
    let random = key
        .cred_random(&stored_credential, false)
        .expect("the credential secret");

    assert_eq!(
        signed
            .expect("the key signs")
            .prf()
            .map(|results| results.first().to_vec()),
        Some(hmac(&random, &prf_salt(b"held")))
    );
}

#[test]
fn large_blob_read_with_credential_key() {
    let id = credential(0x31, 16);
    let blob: Vec<u8> = (0..2000u32)
        .map(|at| (at.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let key = FakeKey::new(Kind::Ctap21)
        .with_resident(RP, id.clone(), User::new(*b"owner"))
        .with_large_blob(&id, &blob);

    let (signed, _) = drive(
        Fido::with_transport(key)
            .assert(
                AssertionRequest::new(ORIGIN, RP, CHALLENGE)
                    .allow(id)
                    .large_blob_read(),
            )
            .expect("valid"),
        SILENT,
    );

    assert_eq!(
        signed.expect("the key signs").large_blob(),
        Some(blob.as_slice())
    );
}

#[test]
fn large_blob_requires_discoverable_credential() {
    let cases = [
        (
            ResidentKey::Required,
            LargeBlobSupport::Required,
            Some(true),
        ),
        (
            ResidentKey::Discouraged,
            LargeBlobSupport::Preferred,
            Some(false),
        ),
        (ResidentKey::Discouraged, LargeBlobSupport::Required, None),
    ];

    for (resident, support, supported) in cases {
        let (made, _) = drive(
            Fido::with_transport(FakeKey::new(Kind::Ctap21))
                .register(registration().resident_key(resident).large_blob(support))
                .expect("valid"),
            SILENT,
        );

        match (made, supported) {
            (Ok(registration), Some(expected)) => {
                assert_eq!(registration.large_blob_supported(), Some(expected));
            }
            (Err(Error::ExtensionUnsupported("largeBlob")), None) => {}
            (outcome, _) => panic!("{resident:?} {support:?}: {outcome:?}"),
        }
    }
}

#[test]
fn rejects_contradictory_extensions() {
    let fido = Fido::with_transport(FakeKey::new(Kind::Ctap21));
    let id = credential(0x01, 16);
    let cases = [
        AssertionRequest::new(ORIGIN, RP, CHALLENGE)
            .allow(id.clone())
            .prf(PrfValues::new(*b"a"))
            .hmac_secret([0; 32], None),
        AssertionRequest::new(ORIGIN, RP, CHALLENGE).prf_for(id.clone(), PrfValues::new(*b"a")),
        AssertionRequest::new(ORIGIN, RP, CHALLENGE)
            .allow(id)
            .prf_for(credential(0x99, 16), PrfValues::new(*b"a")),
    ];

    for request in cases {
        assert!(matches!(
            fido.assert(request),
            Err(Error::InvalidRequest(_))
        ));
    }
}
