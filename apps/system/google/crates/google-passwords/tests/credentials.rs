use google_passwords::Credential;
use google_sync::proto::{Note, Notes, PasswordSpecificsData};

fn specifics(realm: &str, origin: &str) -> PasswordSpecificsData {
    PasswordSpecificsData {
        signon_realm: Some(realm.to_owned()),
        origin: Some(origin.to_owned()),
        username_value: Some("rikka".to_owned()),
        password_value: Some("secret".to_owned()),
        date_created: Some(13_344_473_600_000_000),
        ..PasswordSpecificsData::default()
    }
}

#[test]
fn sites_are_named_and_grouped_by_kind() {
    let cases = [
        (
            "https://www.accounts.example.co.uk/",
            "https://www.accounts.example.co.uk/login",
            ("accounts.example.co.uk", "example.co.uk", false),
            "https://www.accounts.example.co.uk/login",
        ),
        (
            "android://hash==@com.example.app/",
            "android://hash==@com.example.app/",
            ("com.example.app", "com.example.app", true),
            "",
        ),
        (
            "http://192.168.0.1:8080/",
            "",
            ("192.168.0.1", "192.168.0.1", false),
            "http://192.168.0.1:8080/",
        ),
    ];
    for (realm, origin, (name, group, is_app), address) in cases {
        let credential =
            Credential::from_specifics("tag", &specifics(realm, origin)).expect("a credential");
        assert_eq!(credential.site.name, name, "{realm}");
        assert_eq!(credential.site.group, group, "{realm}");
        assert_eq!(credential.site.is_app, is_app, "{realm}");
        assert_eq!(credential.site.address, address, "{realm}");
    }
}

#[test]
fn credential_reads_note_dates_and_hides_secrets_from_debug() {
    let mut data = specifics("https://example.com/", "https://example.com/");
    data.notes = Some(Notes {
        note: vec![Note {
            value: Some("backup codes".to_owned()),
            ..Note::default()
        }],
    });

    let credential = Credential::from_specifics("tag", &data).expect("a credential");

    assert_eq!(credential.note, "backup codes");
    assert_eq!(credential.created_ms, Some(1_700_000_000_000));
    assert_eq!(credential.used_ms, None);
    assert!(credential.matches("exam"));
    assert!(credential.matches("rik"));
    assert!(!credential.matches("other"));
    let printed = format!("{credential:?}");
    assert!(!printed.contains("secret") && !printed.contains("backup"));
}

#[test]
fn never_saved_sites_are_not_credentials() {
    let mut data = specifics("https://example.com/", "https://example.com/");
    data.blacklisted = Some(true);

    assert!(Credential::from_specifics("tag", &data).is_none());
}
