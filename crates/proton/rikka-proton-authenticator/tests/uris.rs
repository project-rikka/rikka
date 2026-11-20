use rikka_proton_authenticator::{
    Algorithm, Base32, Entry, Error, Hotp, MigrationPayload, Otp, Totp,
};

const MIGRATION: &str = "otpauth-migration://offline?data=CjUKBWYkQUSTEgdNWUxBQkVMGghNWUlTU1VFUiACKAIwAkITNjE5NGJjMTczNzcyNzc5ODc5MxACGAEgAA%3D%3D";

#[test]
fn otp_uris_round_trip() {
    for (uri, expected) in [
        (
            "otpauth://totp/Example%20Label?secret=MYSECRET&issuer=Proton&algorithm=SHA256&digits=8&period=15",
            Otp::Totp(Totp {
                label: Some("Example Label".to_owned()),
                issuer: Some("Proton".to_owned()),
                secret: "MYSECRET".to_owned(),
                algorithm: Algorithm::Sha256,
                digits: 8,
                period: 15,
            }),
        ),
        (
            "otpauth://hotp/Label?secret=MYSECRET&issuer=Proton&algorithm=SHA512&digits=7&counter=42",
            Otp::Hotp(Hotp {
                label: Some("Label".to_owned()),
                issuer: Some("Proton".to_owned()),
                secret: "MYSECRET".to_owned(),
                algorithm: Algorithm::Sha512,
                digits: 7,
                counter: 42,
            }),
        ),
        (
            "otpauth://totp/label%3Awith%3Acolons?secret=MYSECRET&algorithm=SHA1&digits=6&period=30",
            Otp::Totp(Totp {
                label: Some("label:with:colons".to_owned()),
                secret: "MYSECRET".to_owned(),
                ..Totp::default()
            }),
        ),
    ] {
        let otp = Otp::parse(uri).unwrap();
        assert_eq!(otp, expected);
        assert_eq!(otp.to_uri(), uri);
    }

    let defaults = Otp::parse("otpauth://totp/Proton:person@proton.me?secret=MYSECRET").unwrap();
    assert_eq!(defaults.label(), Some("person@proton.me"));
    assert_eq!(defaults.issuer(), Some("Proton"));
    assert_eq!(
        defaults.to_uri(),
        "otpauth://totp/person%40proton.me?secret=MYSECRET&issuer=Proton&algorithm=SHA1&digits=6&period=30"
    );
}

type ErrorMatcher = fn(&Error) -> bool;

#[test]
fn rejects_invalid_uris() {
    let cases: [(&str, ErrorMatcher); 9] = [
        ("MYSECRET", |error| matches!(error, Error::MalformedUri)),
        ("https://proton.me", |error| {
            matches!(error, Error::UnsupportedScheme { .. })
        }),
        ("otpauth://yubikey/Label?secret=MYSECRET", |error| {
            matches!(error, Error::UnsupportedType { .. })
        }),
        ("otpauth://totp/Label", |error| {
            matches!(error, Error::MissingSecret)
        }),
        ("otpauth://totp/Label?secret=", |error| {
            matches!(error, Error::EmptySecret)
        }),
        ("otpauth://totp/Label?secret=MYSECRET&digits=0", |error| {
            matches!(error, Error::InvalidDigits { .. })
        }),
        ("otpauth://totp/Label?secret=MYSECRET&period=0", |error| {
            matches!(error, Error::InvalidPeriod { .. })
        }),
        (
            "otpauth://hotp/Label?secret=MYSECRET&counter=soon",
            |error| matches!(error, Error::InvalidCounter { .. }),
        ),
        (
            "otpauth://totp/Label?secret=MYSECRET&algorithm=MD5",
            |error| matches!(error, Error::UnsupportedAlgorithm { .. }),
        ),
    ];
    for (uri, expected) in cases {
        let error = Otp::parse(uri).unwrap_err();
        assert!(expected(&error), "{uri}: {error:?}");
    }
}

#[test]
fn entry_round_trips_and_rejects_empty() {
    let entry = Entry::from_uri(
        "7f1e",
        "otpauth://totp/Proton:person@proton.me?secret=MYSECRET&algorithm=SHA256&digits=8&period=15",
    )
    .unwrap()
    .with_note("a note");

    let decoded = Entry::decode(&entry.encode()).unwrap();
    assert_eq!(decoded, entry);
    assert_eq!(decoded.note.as_deref(), Some("a note"));
    assert_eq!(decoded.secret(), "MYSECRET");

    assert!(matches!(Entry::decode(&[]).unwrap_err(), Error::EmptyEntry));
    assert!(matches!(
        Entry::decode(&[0x0a, 0x05, 0x00]).unwrap_err(),
        Error::Malformed { .. }
    ));
}

#[test]
fn reads_google_authenticator_export() {
    let payload = MigrationPayload::from_uri(MIGRATION).unwrap();
    let parameters = &payload.otp_parameters[0];
    assert_eq!(Base32::encode(&parameters.secret), "MYSECRET");

    let otps = payload.otps();
    assert_eq!(otps.len(), 1);
    assert_eq!(
        otps[0].as_ref().unwrap(),
        &Otp::Totp(Totp {
            label: Some("MYLABEL".to_owned()),
            issuer: Some("MYISSUER".to_owned()),
            secret: "MYSECRET".to_owned(),
            algorithm: Algorithm::Sha256,
            digits: 8,
            period: 30,
        })
    );

    for uri in [
        "otpauth-migration://offline?data=not%20base64%21",
        "otpauth-migration://offline",
    ] {
        assert!(matches!(
            MigrationPayload::from_uri(uri).unwrap_err(),
            Error::MalformedPayload
        ));
    }
}
