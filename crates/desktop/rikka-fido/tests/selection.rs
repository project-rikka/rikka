use std::time::Duration;

use rikka_fido::{AssertionRequest, Event, Fido, RegistrationRequest, RelyingParty, User};
use rikka_test_fido::{FakeKey, FakeKeys, Kind};

use crate::support::{CHALLENGE, ORIGIN, RP, SILENT, credential, drive};

#[test]
fn touched_key_wins_others_cancelled() {
    for kind in [Kind::Ctap21, Kind::Ctap20, Kind::U2f] {
        let idle = FakeKey::new(Kind::Ctap21).named("idle").unanswered();
        let touched = FakeKey::new(kind)
            .named("touched")
            .with_touch_delay(Duration::from_millis(300));
        let fido = Fido::with_transport(FakeKeys::new([idle.clone(), touched.clone()]));

        let request = RegistrationRequest::new(
            ORIGIN,
            RelyingParty::new(RP),
            User::new(*b"user").with_name("user"),
            CHALLENGE,
        );
        let (made, events) = drive(fido.register(request).expect("valid"), SILENT);
        let registration = made.unwrap_or_else(|error| panic!("{kind:?}: {error}"));

        assert_eq!(events.first(), Some(&Event::Choose), "{kind:?}");
        assert_eq!(
            touched.credential_ids(RP),
            vec![registration.credential_id().to_vec()],
            "{kind:?}"
        );
        assert!(idle.credential_ids(RP).is_empty(), "{kind:?}");
        assert!(idle.was_cancelled(), "{kind:?}");
    }
}

#[test]
fn only_key_with_credential_signs() {
    let id = credential(0x70, 32);
    let stranger = FakeKey::new(Kind::Ctap21).named("stranger").unanswered();
    let holder = FakeKey::new(Kind::U2f)
        .named("holder")
        .with_credential(RP, id.clone());
    let fido = Fido::with_transport(FakeKeys::new([stranger, holder]));

    let (signed, events) = drive(
        fido.assert(AssertionRequest::new(ORIGIN, RP, CHALLENGE).allow(id.clone()))
            .expect("valid"),
        SILENT,
    );

    assert_eq!(signed.expect("the holder signs").credential_id(), id);
    assert!(!events.contains(&Event::Choose));
}
