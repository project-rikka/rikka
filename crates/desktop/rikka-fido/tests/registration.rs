use rikka_fido::cbor::Value;
use rikka_fido::{
    Algorithm, AttestationConveyance, Error, Event, Fido, RegistrationRequest, RelyingParty,
    ResidentKey, User, UserVerification,
};
use rikka_test_fido::{FakeKey, Kind};

use crate::support::{Answers, CHALLENGE, ORIGIN, RP, SILENT, credential, drive, sha256};

const MAKE_CREDENTIAL: u8 = 0x01;

fn request() -> RegistrationRequest {
    RegistrationRequest::new(
        ORIGIN,
        RelyingParty::new(RP).with_name("Example"),
        User::new(*b"user-1")
            .with_name("person@example.com")
            .with_display_name("Person"),
        CHALLENGE,
    )
}

#[test]
fn ctap2_makes_discoverable_credential() {
    let key = FakeKey::new(Kind::Ctap21);

    let (made, events) = drive(
        Fido::with_transport(key.clone())
            .register(request().resident_key(ResidentKey::Required))
            .expect("valid"),
        SILENT,
    );
    let registration = made.expect("the key makes a credential");

    assert_eq!(events, vec![Event::Touch]);
    assert!(registration.is_discoverable());
    assert_eq!(registration.algorithm(), Algorithm::ES256);
    assert_eq!(
        key.credential_ids(RP),
        vec![registration.credential_id().to_vec()]
    );
    assert!(
        registration
            .client_data_json()
            .starts_with(r#"{"type":"webauthn.create","#)
    );

    let object = Value::decode(registration.attestation_object()).expect("CBOR");
    assert_eq!(object.get("fmt"), Some(&Value::from("none")));
    assert_eq!(
        object.get("authData").and_then(Value::as_bytes),
        Some(registration.authenticator_data())
    );
    assert_eq!(registration.authenticator_data()[..32], sha256(RP));
    let public_key = Value::decode(registration.public_key()).expect("a COSE key");
    assert_eq!(public_key.get(3), Some(&Value::from(-7)));

    let ctap_request = key.requests(MAKE_CREDENTIAL).pop().expect("one request");
    assert_eq!(
        ctap_request.get(0x07).and_then(|options| options.get("rk")),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        ctap_request
            .get(0x03)
            .and_then(|user| user.get("displayName")),
        Some(&Value::from("Person"))
    );
}

#[test]
fn ctap20_requests_required_pin() {
    let key = FakeKey::new(Kind::Ctap20).with_pin("4321");

    let (made, events) = drive(
        Fido::with_transport(key.clone())
            .register(request().user_verification(UserVerification::Discouraged))
            .expect("valid"),
        Answers {
            pins: &["4321"],
            account: 0,
        },
    );

    assert!(made.is_ok(), "{made:?}");
    assert!(matches!(
        events.first(),
        Some(Event::Pin { wrong: false, .. })
    ));
    assert_eq!(
        key.requests(MAKE_CREDENTIAL).len(),
        2,
        "retried with the PIN"
    );
}
#[test]
fn none_conveyance_strips_attestation() {
    let cases = [
        (
            Kind::Ctap21,
            AttestationConveyance::None,
            "none",
            [0; 16],
            0,
        ),
        (
            Kind::Ctap21,
            AttestationConveyance::Direct,
            "none",
            [0x11; 16],
            0,
        ),
        (Kind::U2f, AttestationConveyance::None, "none", [0; 16], 0),
        (
            Kind::U2f,
            AttestationConveyance::Indirect,
            "fido-u2f",
            [0; 16],
            2,
        ),
    ];

    for (kind, conveyance, format, aaguid, statement_fields) in cases {
        let (made, _) = drive(
            Fido::with_transport(FakeKey::new(kind))
                .register(request().attestation(conveyance))
                .expect("valid"),
            SILENT,
        );
        let registration = made.expect("the key registers");
        let object = Value::decode(registration.attestation_object()).expect("CBOR");

        assert_eq!(registration.format(), format, "{conveyance:?}");
        assert_eq!(object.get("fmt"), Some(&Value::from(format)));
        assert_eq!(
            object
                .get("attStmt")
                .and_then(Value::as_map)
                .map(<[(Value, Value)]>::len),
            Some(statement_fields),
            "{conveyance:?}"
        );
        assert_eq!(&registration.authenticator_data()[37..53], &aaguid);
        assert_eq!(
            object.get("authData").and_then(Value::as_bytes),
            Some(registration.authenticator_data())
        );
    }
}

#[test]
fn u2f_registers_with_fido_u2f_attestation() {
    let key = FakeKey::new(Kind::U2f);

    let (made, _) = drive(
        Fido::with_transport(key.clone())
            .register(request().attestation(AttestationConveyance::Direct))
            .expect("valid"),
        SILENT,
    );
    let registration = made.expect("the U2F key registers");

    assert_eq!(registration.format(), "fido-u2f");
    assert_eq!(registration.credential_id().len(), 48);
    assert_eq!(
        key.credential_ids(RP),
        vec![registration.credential_id().to_vec()]
    );

    let object = Value::decode(registration.attestation_object()).expect("CBOR");
    let statement = object.get("attStmt").expect("a statement");
    assert_eq!(
        statement
            .get("x5c")
            .and_then(Value::as_array)
            .map(<[Value]>::len),
        Some(1)
    );
    assert!(statement.get("sig").and_then(Value::as_bytes).is_some());

    let data = registration.authenticator_data();
    assert_eq!(data[32], 0x41);
    let id_length = usize::from(u16::from_be_bytes([data[53], data[54]]));
    assert_eq!(&data[55..55 + id_length], registration.credential_id());
    let public_key = Value::decode(&data[55 + id_length..]).expect("a COSE key");
    assert_eq!(
        public_key.get(-2).and_then(Value::as_bytes),
        Some(&[0x07; 32][..])
    );
}

#[test]
fn excluded_credential_stops_registration() {
    for kind in [Kind::Ctap21, Kind::U2f] {
        let id = credential(0x0e, 48);
        let key = FakeKey::new(kind).with_credential(RP, id.clone());

        let (made, _) = drive(
            Fido::with_transport(key)
                .register(request().exclude(id))
                .expect("valid"),
            SILENT,
        );

        assert!(
            matches!(made, Err(Error::CredentialExcluded)),
            "{kind:?}: {made:?}"
        );
    }
}
