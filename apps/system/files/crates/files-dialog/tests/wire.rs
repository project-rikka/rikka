use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use files_dialog::{Answer, Ask, Kind, Outcome};
use rikka_types::WindowIdentifier;

fn broken() -> PathBuf {
    PathBuf::from(OsString::from_vec(b"/tmp/\xff\xfe.png".to_vec()))
}

#[test]
fn non_utf8_name_round_trips() {
    let ask = Ask {
        folder: Some(broken()),
        ..Ask::new(Kind::Open)
    };
    let mut written = Vec::new();
    rikka_portal::write(&ask, &mut written).unwrap();
    assert_eq!(Ask::read(written.as_slice()).unwrap(), ask);

    let outcome = Outcome::Chosen(Answer::new(vec![broken()]));
    let mut written = Vec::new();
    outcome.write(&mut written).unwrap();
    assert_eq!(Outcome::read(written.as_slice()).unwrap(), outcome);
}

#[test]
fn parent_window_round_trips() {
    for parent in [
        WindowIdentifier::Wayland("4f2a91".into()),
        WindowIdentifier::X11(0x3a0_0004),
    ] {
        let ask = Ask {
            parent: Some(parent),
            modal: false,
            ..Ask::new(Kind::Save)
        };
        let mut written = Vec::new();
        rikka_portal::write(&ask, &mut written).unwrap();
        assert_eq!(Ask::read(written.as_slice()).unwrap(), ask);
    }
    assert!(Ask::read(br#"{"kind":"open","parent":"mir:1"}"#.as_slice()).is_err());
}

#[test]
fn missing_fields_use_defaults() {
    let ask = Ask::read(br#"{"kind":"save"}"#.as_slice()).unwrap();
    assert_eq!(ask, Ask::new(Kind::Save));
    assert!(ask.modal);
}
