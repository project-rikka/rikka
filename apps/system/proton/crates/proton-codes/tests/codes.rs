use proton_codes::{CodeStore, Setup, State, StoredCode, code_at};
use rikka_proton_authenticator::{Algorithm, Entry, Otp, RemoteEntry, SyncOutcome};

const URI: &str = "otpauth://totp/person?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Proton";

fn now() -> jiff::Timestamp {
    jiff::Timestamp::from_second(1_700_000_000).expect("a moment")
}

fn codes() -> CodeStore {
    let mut codes = CodeStore::new();
    codes.import(URI, None, now()).expect("it is a code");
    codes
}

#[test]
fn rejects_invalid_code() {
    let mut codes = CodeStore::new();

    assert!(codes.import("hello", None, now()).is_err());
    assert!(codes.all().is_empty());
}

#[test]
fn synced_code_kept_until_server_deletes() {
    let mut codes = codes();
    let id = codes.all()[0].id().to_owned();
    let entry = Entry::from_uri(id.clone(), URI).expect("it is a code");
    let at = now();
    codes.apply_sync(&SyncOutcome {
        entries: vec![RemoteEntry {
            remote_id: "remote-1".to_owned(),
            revision: 2,
            entry,
            position: Some(3),
            created_at: at,
            modified_at: at,
        }],
        ..SyncOutcome::default()
    });
    assert_eq!(codes.all()[0].state(), State::Synced);
    assert_eq!(codes.all()[0].position(), 3);
    assert!(codes.pending().is_empty());

    codes.remove(&id);
    assert_eq!(codes.all()[0].state(), State::PendingDelete);
    assert!(codes.filtered().is_empty());
    assert_eq!(codes.pending().len(), 1);

    codes.apply_sync(&SyncOutcome {
        removed: vec![id],
        ..SyncOutcome::default()
    });
    assert!(codes.all().is_empty());
}

#[test]
fn imports_google_authenticator_export() {
    let mut codes = CodeStore::new();
    let uri = "otpauth-migration://offline?data=CjUKBWYkQUSTEgdNWUxBQkVMGghNWUlTU1VFUiACKAIwAkITNjE5NGJjMTczNzcyNzc5ODc5MxACGAEgAA%3D%3D";

    let added = codes.import(uri, None, now()).expect("the export is read");

    assert_eq!(added, 1);
    assert_eq!(codes.all().len(), 1);
}

#[test]
fn totp_matches_rfc_6238() {
    let at = jiff::Timestamp::from_second(59).expect("a moment");
    let uri = "otpauth://totp/person?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&digits=8";

    let code = code_at(uri, at).expect("the code is worked out");

    assert_eq!(code.now, "94287082");
    assert_eq!(code.remaining, 1);
    assert_eq!(code.period, 30);
}

#[test]
fn setup_round_trips_to_otp() {
    let setup = Setup {
        title: " person ".to_owned(),
        secret: "gezd gnbv gy3t qojq".to_owned(),
        issuer: "Proton".to_owned(),
        digits: 8,
        period: 60,
        algorithm: Algorithm::Sha256,
        ..Setup::default()
    };

    let otp = setup.otp().expect("it is a code");

    assert_eq!(otp.label(), Some("person"));
    assert_eq!(otp.issuer(), Some("Proton"));
    assert_eq!(otp.secret(), "GEZDGNBVGY3TQOJQ");
    assert_eq!(otp.digits(), 8);
    assert_eq!(otp.period(), Some(60));
    assert_eq!(otp.algorithm(), Algorithm::Sha256);
}

fn remote(id: &str, uri: &str) -> RemoteEntry {
    RemoteEntry {
        remote_id: format!("remote-{id}"),
        revision: 1,
        entry: Entry::from_uri(id.to_owned(), uri).expect("it is a code"),
        position: Some(0),
        created_at: now(),
        modified_at: now(),
    }
}

#[test]
fn recall_lays_pending_codes_over_synced_ones() {
    let synced = [
        StoredCode::from_remote(&remote("kept", URI)),
        StoredCode::from_remote(&remote("edited", URI)),
        StoredCode::from_remote(&remote("dropped", URI)),
    ];
    let mut earlier = CodeStore::new();
    earlier.recall(synced.to_vec(), Vec::new());
    let edited = Otp::parse("otpauth://totp/edited?secret=GEZDGNBVGY3TQOJQ&issuer=Edited")
        .expect("it is a code");
    earlier
        .change("edited", &edited, Some("changed".to_owned()), now())
        .expect("the code exists");
    earlier.remove("dropped");
    earlier
        .import(
            "otpauth://totp/local?secret=GEZDGNBVGY3TQOJQ&issuer=Local",
            None,
            now(),
        )
        .expect("it is a code");
    let pending = earlier.pending();
    assert_eq!(pending.len(), 3);

    let mut codes = CodeStore::new();
    codes
        .import(
            "otpauth://totp/other?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Other",
            None,
            now(),
        )
        .expect("it is a code");
    codes.recall(synced.to_vec(), pending);

    assert_eq!(codes.all().len(), 5);
    assert_eq!(
        codes.get("kept").map(StoredCode::state),
        Some(State::Synced)
    );
    let changed = codes.get("edited").expect("the edit is recalled");
    assert_eq!(changed.state(), State::Pending);
    assert_eq!(changed.issuer(), "Edited");
    assert_eq!(changed.note(), Some("changed"));
    assert_eq!(
        codes.get("dropped").map(StoredCode::state),
        Some(State::PendingDelete)
    );
    assert_eq!(codes.filtered().len(), 4);
    assert_eq!(codes.pending().len(), 4);
}
