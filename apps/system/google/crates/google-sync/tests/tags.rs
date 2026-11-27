use google_sync::proto::PasswordSpecificsData;
use google_sync::{
    client_tag, client_tag_hash, unix_ms_from_windows_micros, windows_micros_from_unix_ms,
};

#[test]
fn client_tag_escapes_like_the_reference() {
    let data = PasswordSpecificsData {
        origin: Some("https://accounts.example.com/login?a=1".to_owned()),
        username_element: Some("user name".to_owned()),
        username_value: Some("me@example.com".to_owned()),
        password_element: Some("pass|word".to_owned()),
        signon_realm: Some("https://accounts.example.com/".to_owned()),
        ..PasswordSpecificsData::default()
    };

    assert_eq!(
        client_tag(&data),
        "https%3A//accounts.example.com/login%3Fa=1|user%20name|me@example.com|pass%7Cword|\
         https%3A//accounts.example.com/"
    );
    assert_eq!(client_tag_hash(&data).len(), 28);
}

#[test]
fn client_tag_follows_only_identifying_fields() {
    let mut data = PasswordSpecificsData {
        origin: Some("https://example.com/".to_owned()),
        signon_realm: Some("https://example.com/".to_owned()),
        username_value: Some("rikka".to_owned()),
        password_value: Some("one".to_owned()),
        ..PasswordSpecificsData::default()
    };
    let tag = client_tag_hash(&data);

    data.password_value = Some("two".to_owned());
    assert_eq!(client_tag_hash(&data), tag);
    data.username_value = Some("other".to_owned());
    assert_ne!(client_tag_hash(&data), tag);
}

#[test]
fn windows_epoch_times_convert_both_ways() {
    let unix_ms = 1_700_000_000_000;

    let micros = windows_micros_from_unix_ms(unix_ms);

    assert_eq!(micros, 13_344_473_600_000_000);
    assert_eq!(unix_ms_from_windows_micros(micros), unix_ms);
}
