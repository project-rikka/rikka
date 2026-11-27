use google_mail::{Address, Error};

#[test]
fn header_lists_split_on_real_separators_only() {
    let parsed = Address::parse_list(
        "\"Sato, Rikka\" <rikka@example.com>, plain@example.com (work), \
         =?UTF-8?B?5bGx55Sw?= =?UTF-8?B?5aSq6YOO?= <taro@example.jp>, \
         =?ISO-2022-JP?B?GyRCRnxLXDhsGyhC?= <jp@example.jp>, \
         =?utf-8?Q?J=C3=BCrgen_M?= <j@example.de>, \"Quoted \\\"Q\\\"\" <q@example.com>",
    );

    let pairs: Vec<(&str, &str)> = parsed
        .iter()
        .map(|address| (address.name.as_str(), address.email.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("Sato, Rikka", "rikka@example.com"),
            ("", "plain@example.com"),
            ("山田太郎", "taro@example.jp"),
            ("日本語", "jp@example.jp"),
            ("Jürgen M", "j@example.de"),
            ("Quoted \"Q\"", "q@example.com"),
        ]
    );
    assert_eq!(parsed[0].label(), "Sato, Rikka");
    assert_eq!(parsed[1].label(), "plain@example.com");
    assert_eq!(parsed[1].short(), "plain");
}

#[test]
fn typed_recipients_must_be_deliverable() {
    let typed = Address::typed_list("a@example.com; Rikka <b@example.org>").expect("recipients");

    assert_eq!(typed.len(), 2);
    assert_eq!(typed[1].to_string(), "Rikka <b@example.org>");
    for wrong in ["rikka", "a@b", "two words@example.com", "Name <>"] {
        assert!(
            matches!(Address::typed_list(wrong), Err(Error::Address(_))),
            "{wrong}"
        );
    }
    assert!(
        Address::typed_list(" , ")
            .expect("nothing typed")
            .is_empty()
    );
}
