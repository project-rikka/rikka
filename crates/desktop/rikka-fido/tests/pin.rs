use rikka_fido::{AssertionRequest, Error, Event, Fido, NewPinReason, UserVerification};
use rikka_test_fido::{FakeKey, Kind};

use crate::support::{Answers, CHALLENGE, ORIGIN, RP, credential, drive};

const PIN: &str = "1234";
const GET_ASSERTION_PERMISSION: u64 = 0x02;

fn pin_event(retries: u32, wrong: bool) -> Event {
    Event::Pin {
        retries,
        min_length: 4,
        wrong,
    }
}

#[test]
fn pin_unlocks_uv_in_every_protocol() {
    let cases = [
        (Kind::Ctap21, &[2, 1][..], true),
        (Kind::Ctap21, &[1][..], true),
        (Kind::Ctap20, &[1][..], false),
    ];

    for (kind, protocols, scoped) in cases {
        let id = credential(0x61, 40);
        let key = FakeKey::new(kind)
            .with_protocols(protocols)
            .with_pin(PIN)
            .with_credential(RP, id.clone());
        let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE)
            .allow(id)
            .user_verification(UserVerification::Required);

        let (signed, events) = drive(
            Fido::with_transport(key.clone())
                .assert(request)
                .expect("valid"),
            Answers {
                pins: &["12", "9999", PIN],
                account: 0,
            },
        );
        let assertion = signed.unwrap_or_else(|error| panic!("{kind:?} {protocols:?}: {error}"));

        assert_eq!(
            events,
            vec![
                pin_event(8, false),
                pin_event(8, true),
                pin_event(7, true),
                Event::Touch
            ],
            "{kind:?} {protocols:?}"
        );
        assert_eq!(
            assertion.authenticator_data()[32] & 0x04,
            0x04,
            "{kind:?} {protocols:?}: the user was verified"
        );
        assert_eq!(key.pin_retries(), 8);
        let expected = if scoped {
            vec![(GET_ASSERTION_PERMISSION, RP.to_owned())]
        } else {
            Vec::new()
        };
        assert_eq!(key.permissions(), expected, "{kind:?} {protocols:?}");
    }
}

#[test]
fn uv_follows_request() {
    let cases = [
        (UserVerification::Discouraged, false),
        (UserVerification::Preferred, true),
    ];

    for (verification, asks) in cases {
        let id = credential(0x07, 32);
        let key = FakeKey::new(Kind::Ctap21)
            .with_pin(PIN)
            .with_credential(RP, id.clone());
        let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE)
            .allow(id)
            .user_verification(verification);

        let (signed, events) = drive(
            Fido::with_transport(key).assert(request).expect("valid"),
            Answers {
                pins: &[PIN],
                account: 0,
            },
        );
        let flags = signed.expect("the key signs").authenticator_data()[32];

        assert_eq!(
            events.contains(&pin_event(8, false)),
            asks,
            "{verification:?}"
        );
        assert_eq!(flags & 0x04 != 0, asks, "{verification:?}");
    }
}

#[test]
fn wrong_pins_block_key() {
    let id = credential(0x33, 32);
    let key = FakeKey::new(Kind::Ctap21)
        .with_pin(PIN)
        .with_credential(RP, id.clone());
    let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE)
        .allow(id)
        .user_verification(UserVerification::Required);

    let (signed, _) = drive(
        Fido::with_transport(key.clone())
            .assert(request)
            .expect("valid"),
        Answers {
            pins: &["0000"; 8],
            account: 0,
        },
    );

    assert!(matches!(signed, Err(Error::PinBlocked)), "{signed:?}");
    assert_eq!(key.pin_retries(), 0);
}

fn new_pin_event(min_length: usize, reason: NewPinReason, rejected: bool) -> Event {
    Event::NewPin {
        min_length,
        reason,
        rejected,
    }
}

#[test]
fn sets_missing_pin_when_uv_required() {
    let cases = [
        (Kind::Ctap21, &[2, 1][..]),
        (Kind::Ctap21, &[1][..]),
        (Kind::Ctap20, &[1][..]),
    ];

    for (kind, protocols) in cases {
        let id = credential(0x44, 32);
        let key = FakeKey::new(kind)
            .with_protocols(protocols)
            .with_credential(RP, id.clone());
        let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE)
            .allow(id)
            .user_verification(UserVerification::Required);

        let (signed, events) = drive(
            Fido::with_transport(key.clone())
                .assert(request)
                .expect("valid"),
            Answers {
                pins: &["12", "5678"],
                account: 0,
            },
        );
        let flags = signed
            .unwrap_or_else(|error| panic!("{kind:?} {protocols:?}: {error}"))
            .authenticator_data()[32];

        assert_eq!(
            events,
            vec![
                new_pin_event(4, NewPinReason::NotSet, false),
                new_pin_event(4, NewPinReason::NotSet, true),
                Event::Touch
            ],
            "{kind:?} {protocols:?}"
        );
        assert_eq!(key.pin().as_deref(), Some("5678"));
        assert_eq!(flags & 0x04, 0x04, "the user was verified");
    }
}

#[test]
fn forced_pin_change_precedes_signing() {
    let id = credential(0x45, 32);
    let key = FakeKey::new(Kind::Ctap21)
        .with_pin(PIN)
        .with_min_pin_length(6)
        .forcing_pin_change()
        .with_credential(RP, id.clone());
    let request = AssertionRequest::new(ORIGIN, RP, CHALLENGE)
        .allow(id)
        .user_verification(UserVerification::Required);

    let (signed, events) = drive(
        Fido::with_transport(key.clone())
            .assert(request)
            .expect("valid"),
        Answers {
            pins: &[PIN, "4321", "654321"],
            account: 0,
        },
    );

    assert!(signed.is_ok(), "{signed:?}");
    assert_eq!(
        events,
        vec![
            Event::Pin {
                retries: 8,
                min_length: 6,
                wrong: false
            },
            new_pin_event(6, NewPinReason::ChangeRequired, false),
            new_pin_event(6, NewPinReason::ChangeRequired, true),
            Event::Touch
        ]
    );
    assert_eq!(key.pin().as_deref(), Some("654321"));
}

#[test]
fn authenticator_manages_pin() {
    for protocols in [&[2, 1][..], &[1][..]] {
        let key = FakeKey::new(Kind::Ctap21).with_protocols(protocols);
        let fido = Fido::with_transport(key.clone());
        let mut authenticator = fido.open(&key.device()).expect("the key opens");

        assert!(matches!(
            authenticator.set_pin("12"),
            Err(Error::PinPolicyViolation { min_length: 4 })
        ));
        assert!(matches!(
            authenticator.change_pin(PIN, "5678"),
            Err(Error::PinNotSet)
        ));
        authenticator.set_pin(PIN).expect("the PIN is set");
        assert_eq!(key.pin().as_deref(), Some(PIN), "{protocols:?}");
        assert_eq!(
            authenticator.info().and_then(|info| info.has_pin()),
            Some(true)
        );
        assert!(matches!(
            authenticator.set_pin("9999"),
            Err(Error::PinAlreadySet)
        ));

        assert!(matches!(
            authenticator.change_pin("0000", "5678"),
            Err(Error::WrongPin { retries: 7 })
        ));
        authenticator
            .change_pin(PIN, "5678")
            .expect("the PIN changes");
        assert_eq!(key.pin().as_deref(), Some("5678"), "{protocols:?}");
        assert_eq!(key.pin_retries(), 8);
    }

    let u2f = FakeKey::new(Kind::U2f);
    let mut authenticator = Fido::with_transport(u2f.clone())
        .open(&u2f.device())
        .expect("the key opens");
    assert!(matches!(
        authenticator.set_pin(PIN),
        Err(Error::PinUnsupported)
    ));
}
