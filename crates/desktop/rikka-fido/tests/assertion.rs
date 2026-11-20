use rikka_fido::cbor::Value;
use rikka_fido::{AssertionRequest, Error, Event, Fido, User};
use rikka_test_fido::{FakeKey, FakeKeys, Kind};

use crate::support::{Answers, CHALLENGE, ORIGIN, RP, SILENT, credential, drive, sha256};

const GET_ASSERTION: u8 = 0x02;
const APPID: &str = "https://legacy.example.com";

fn expected_client_data() -> String {
    format!(
        r#"{{"type":"webauthn.get","challenge":"WlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlo","origin":"{ORIGIN}","crossOrigin":false}}"#
    )
}

#[test]
fn ctap2_signs_through_noisy_framing() {
    let stored_credential = credential(0x40, 96);
    let other = credential(0x90, 96);
    let key = FakeKey::new(Kind::Ctap21)
        .with_credential(RP, stored_credential.clone())
        .with_touches(3)
        .noisy();

    let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE)
        .allow(other.clone())
        .allow(stored_credential.clone());
    let ceremony = Fido::with_transport(key.clone())
        .assert(request)
        .expect("the request is valid");
    let (signed, events) = drive(ceremony, SILENT);
    let assertion = signed.expect("the key signs");

    assert_eq!(events, vec![Event::Touch]);
    assert_eq!(assertion.credential_id(), stored_credential);
    assert_eq!(assertion.client_data_json(), expected_client_data());
    assert!(!assertion.used_appid());

    let data = assertion.authenticator_data();
    assert_eq!(data[..32], sha256(RP));
    assert_eq!(data[32] & 0x01, 0x01, "the user was present");
    let client_data_hash = sha256(expected_client_data());
    assert_eq!(
        assertion.signature(),
        sha256([data, client_data_hash.as_slice()].concat())
    );

    let ctap_request = key.requests(GET_ASSERTION).pop().expect("one assertion");
    assert_eq!(ctap_request.get(0x01), Some(&Value::from(RP)));
    assert_eq!(
        ctap_request.get(0x02),
        Some(&Value::bytes(client_data_hash))
    );
    let allowed: Vec<&[u8]> = ctap_request
        .get(0x03)
        .and_then(Value::as_array)
        .expect("an allow list")
        .iter()
        .filter_map(|descriptor| descriptor.get("id")?.as_bytes())
        .collect();
    assert_eq!(
        allowed,
        vec![other.as_slice(), stored_credential.as_slice()]
    );
}

#[test]
fn u2f_only_key_signs_via_ctap1() {
    let handle = credential(0x10, 64);
    let key = FakeKey::new(Kind::U2f).with_credential(RP, handle.clone());

    let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE).allow(handle.clone());
    let (signed, events) = drive(
        Fido::with_transport(key.clone())
            .assert(request)
            .expect("valid"),
        SILENT,
    );
    let assertion = signed.expect("the U2F key signs");

    assert_eq!(events, vec![Event::Touch]);
    assert_eq!(assertion.credential_id(), handle);
    let data = assertion.authenticator_data();
    assert_eq!(data.len(), 37);
    assert_eq!(data[..32], sha256(RP));
    assert_eq!(data[32], 0x01);

    let apdus = key.apdus();
    assert_eq!(
        apdus.first().map(|apdu| apdu[2]),
        Some(0x07),
        "check-only first"
    );
    assert!(apdus.iter().any(|apdu| apdu[1] == 0x02 && apdu[2] == 0x03));
}

#[test]
fn finds_appid_credentials() {
    for kind in [Kind::U2f, Kind::Ctap21] {
        let id = credential(0x22, 48);
        let key = FakeKey::new(kind).with_credential(APPID, id.clone());

        let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE)
            .allow(id.clone())
            .appid(APPID);
        let (signed, _) = drive(
            Fido::with_transport(key).assert(request).expect("valid"),
            SILENT,
        );
        let assertion = signed.unwrap_or_else(|error| panic!("{kind:?}: {error}"));

        assert!(assertion.used_appid(), "{kind:?}");
        assert_eq!(
            assertion.authenticator_data()[..32],
            sha256(APPID),
            "{kind:?}"
        );
        assert_eq!(assertion.credential_id(), id, "{kind:?}");
    }
}

#[test]
fn reports_missing_credential() {
    for kind in [Kind::U2f, Kind::Ctap21, Kind::Ctap20] {
        let key = FakeKey::new(kind).with_credential(RP, credential(0x01, 32));
        let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE).allow(credential(0x77, 32));

        let (signed, _) = drive(
            Fido::with_transport(key).assert(request).expect("valid"),
            SILENT,
        );

        assert!(
            matches!(signed, Err(Error::NoCredentials)),
            "{kind:?}: {signed:?}"
        );
    }
}

#[test]
fn searches_long_allow_lists_in_batches() {
    let stored_credential = credential(0xa0, 32);
    let mut request = AssertionRequest::new(ORIGIN, RP, CHALLENGE);
    for seed in 0..5 {
        request = request.allow(credential(seed * 16, 32));
    }
    request = request.allow(stored_credential.clone());
    let key = FakeKey::new(Kind::Ctap21)
        .with_max_credentials(2)
        .with_credential(RP, stored_credential.clone());

    let (signed, _) = drive(
        Fido::with_transport(key.clone())
            .assert(request)
            .expect("valid"),
        SILENT,
    );
    assert_eq!(
        signed.expect("the key signs").credential_id(),
        stored_credential
    );

    let ctap_requests = key.requests(GET_ASSERTION);
    let silent = |request: &Value| {
        request
            .get(0x05)
            .and_then(|options| options.get("up"))
            .and_then(Value::as_bool)
            == Some(false)
    };
    let batches: Vec<usize> = ctap_requests
        .iter()
        .map(|request| {
            request
                .get(0x03)
                .and_then(Value::as_array)
                .map_or(0, <[_]>::len)
        })
        .collect();
    assert_eq!(batches, vec![2, 2, 2, 1]);
    assert!(
        ctap_requests[..3].iter().all(silent),
        "probes ask for no touch"
    );
    assert!(
        !silent(&ctap_requests[3]),
        "the real assertion asks for a touch"
    );
}

#[test]
fn offers_discoverable_accounts() {
    let alice = User::new(*b"alice").with_name("alice@example.com");
    let bob = User::new(*b"bob").with_name("bob@example.com");
    let key = FakeKey::new(Kind::Ctap21)
        .with_resident(RP, credential(0x01, 16), alice.clone())
        .with_resident(RP, credential(0x31, 16), bob.clone());

    let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE);
    let (signed, events) = drive(
        Fido::with_transport(key).assert(request).expect("valid"),
        Answers {
            pins: &[],
            account: 1,
        },
    );
    let assertion = signed.expect("the key signs");

    assert!(events.contains(&Event::Account {
        users: vec![alice, bob.clone()]
    }));
    assert_eq!(assertion.user(), Some(&bob));
    assert_eq!(assertion.credential_id(), credential(0x31, 16));
}

#[test]
fn rejects_requests_outside_origin() {
    let fido = Fido::with_transport(FakeKeys::new([]));
    let cases = [
        (
            AssertionRequest::new("http://example.com", RP, CHALLENGE),
            "origin",
        ),
        (
            AssertionRequest::new("https://example.org", RP, CHALLENGE),
            "rp",
        ),
        (
            AssertionRequest::new("https://example.com", "com", CHALLENGE),
            "rp",
        ),
        (
            AssertionRequest::new(ORIGIN, RP, CHALLENGE).appid("https://evil.test"),
            "appid",
        ),
        (AssertionRequest::new(ORIGIN, RP, Vec::new()), "request"),
        (
            AssertionRequest::new("https://foo.github.io", "github.io", CHALLENGE),
            "rp",
        ),
        (
            AssertionRequest::new("https://login.example.co.uk", "co.uk", CHALLENGE),
            "rp",
        ),
        (
            AssertionRequest::new("https://login.example.co.uk", "example.co.uk", CHALLENGE)
                .appid("https://attacker.co.uk"),
            "appid",
        ),
    ];

    for (request, refusal) in cases {
        let error = fido.assert(request).expect_err(refusal);
        let matched = match refusal {
            "origin" => matches!(error, Error::InvalidOrigin(_)),
            "rp" => matches!(error, Error::RelyingPartyMismatch { .. }),
            "appid" => matches!(error, Error::InvalidAppId(_)),
            _ => matches!(error, Error::InvalidRequest(_)),
        };
        assert!(matched, "{refusal}: {error}");
    }
}

#[test]
fn accepts_rp_within_registrable_domain() {
    let fido = Fido::with_transport(FakeKeys::new([]));
    let cases = [
        AssertionRequest::new("https://foo.github.io", "foo.github.io", CHALLENGE),
        AssertionRequest::new("https://login.example.co.uk", "example.co.uk", CHALLENGE)
            .appid("https://example.co.uk/app-id.json"),
        AssertionRequest::new("http://localhost:8080", "localhost", CHALLENGE),
        AssertionRequest::new("https://ACCOUNT.Example.com", "Example.COM", CHALLENGE),
    ];

    for request in cases {
        let shown = format!("{} {}", request.origin(), request.rp_id());
        let ceremony = fido.assert(request);
        assert!(ceremony.is_ok(), "{shown}: {ceremony:?}");
    }
}

#[test]
fn cancel_during_touch_cancels_key() {
    let id = credential(0x05, 32);
    let key = FakeKey::new(Kind::Ctap21)
        .with_credential(RP, id.clone())
        .unanswered();

    let mut ceremony = Fido::with_transport(key.clone())
        .assert(AssertionRequest::new(ORIGIN, RP, CHALLENGE).allow(id))
        .expect("valid");
    assert_eq!(ceremony.blocking_event(), Some(Event::Touch));
    ceremony.cancel();

    assert!(matches!(ceremony.wait(), Err(Error::Cancelled)));
    assert!(key.was_cancelled());
}

#[test]
fn waits_for_key_until_cancelled() {
    let mut ceremony = Fido::with_transport(FakeKeys::new([]))
        .assert(AssertionRequest::new(ORIGIN, RP, CHALLENGE))
        .expect("valid");

    assert_eq!(ceremony.blocking_event(), Some(Event::Insert));
    ceremony.cancel();
    assert!(matches!(ceremony.wait(), Err(Error::Cancelled)));
}
