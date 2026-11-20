use rikka_proton_pass::{
    AllowedAndroidApp, AndroidSpecific, AutofillMode, AutofillUrl, CardType, Error, ExtraField,
    ExtraFieldContent, ExtraTotp, Item, ItemContent, ItemCreditCard, ItemKind, ItemLogin, Metadata,
    Passkey, PasskeyCreationData, PlatformSpecific,
};

fn roundtrip(item: &Item) -> Item {
    Item::decode(&item.encode()).unwrap()
}

#[test]
fn login_item_round_trips() {
    let login = ItemLogin {
        item_email: "person@proton.me".to_owned(),
        password: "hunter2".to_owned(),
        urls: vec![
            "https://proton.me".to_owned(),
            "https://pass.proton.me".to_owned(),
        ],
        totp_uri: "otpauth://totp/Proton?secret=MYSECRET".to_owned(),
        passkeys: vec![Passkey {
            key_id: "key".to_owned(),
            content: vec![1, 2, 3],
            domain: "proton.me".to_owned(),
            rp_id: "proton.me".to_owned(),
            rp_name: "Proton".to_owned(),
            user_name: "person".to_owned(),
            user_display_name: "Person".to_owned(),
            user_id: vec![4, 5],
            create_time: 1_700_000_000,
            note: "passkey".to_owned(),
            credential_id: vec![6],
            user_handle: vec![7],
            creation_data: Some(PasskeyCreationData {
                os_name: "Rikka".to_owned(),
                os_version: "1".to_owned(),
                device_name: "desk".to_owned(),
                app_version: "2".to_owned(),
            }),
        }],
        item_username: "person".to_owned(),
        autofill_urls: vec![AutofillUrl {
            url: "https://proton.me".to_owned(),
            mode: AutofillMode::ExactPath as i32,
        }],
    };
    let metadata = Metadata {
        name: "Proton".to_owned(),
        note: "a note".to_owned(),
        item_uuid: "7f1e".to_owned(),
        custom_icon: None,
    };
    let mut item = Item::new(metadata, ItemContent::Login(login));
    item.platform_specific = Some(PlatformSpecific {
        android: Some(AndroidSpecific {
            allowed_apps: vec![AllowedAndroidApp {
                package_name: "me.proton.pass".to_owned(),
                hashes: vec!["abc".to_owned()],
                app_name: "Pass".to_owned(),
            }],
        }),
    });
    item.extra_fields = vec![ExtraField {
        field_name: "second factor".to_owned(),
        content: Some(ExtraFieldContent::Totp(ExtraTotp {
            totp_uri: "otpauth://totp/Second?secret=OTHERSECRET".to_owned(),
        })),
    }];

    let decoded = roundtrip(&item);

    assert_eq!(decoded, item);
    assert_eq!(decoded.kind(), Some(ItemKind::Login));
    let login = decoded.login().unwrap();
    assert_eq!(login.username(), Some("person"));
    assert!(login.has_password());
    assert_eq!(
        decoded.totp_uris(),
        vec![
            "otpauth://totp/Proton?secret=MYSECRET",
            "otpauth://totp/Second?secret=OTHERSECRET",
        ]
    );
}

#[test]
fn unknown_enum_value_reads_as_default() {
    let card = ItemCreditCard {
        card_type: 99,
        ..ItemCreditCard::default()
    };
    let item = Item::new(Metadata::default(), ItemContent::CreditCard(card));

    let decoded = roundtrip(&item);
    let card = decoded.credit_card().unwrap();

    assert_eq!(card.card_type, 99);
    assert_eq!(card.card_type(), CardType::Unspecified);
}

#[test]
fn rejects_corrupt_bytes() {
    assert!(matches!(
        Item::decode(&[0x0a, 0x05, 0x00]).unwrap_err(),
        Error::Malformed { .. }
    ));
}
