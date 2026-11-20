use rikka_proton_contacts::{
    Contact, ContactId, Error, Property, VCard, Version, find_duplicates, merge_vcards, parse_vcf,
    prepare_for_saving,
};
use serde_json::json;

const RFC_6350_EXAMPLE: &str = "BEGIN:VCARD\r
VERSION:4.0\r
FN:Simon Perreault\r
N:Perreault;Simon;;;ing. jr,M.Sc.\r
BDAY:--0203\r
ANNIVERSARY:20090808T1430-0500\r
GENDER:M\r
LANG;PREF=1:fr\r
LANG;PREF=2:en\r
ORG;TYPE=work:Viagenie\r
ADR;TYPE=work:;Suite D2-630;2875 Laurier;\r
 Quebec;QC;G1V 2M2;Canada\r
TEL;VALUE=uri;TYPE=\"work,voice\";PREF=1:tel:+1-418-656-9254;ext=102\r
TEL;VALUE=uri;TYPE=\"work,cell,voice,video,text\":tel:+1-418-262-6501\r
EMAIL;TYPE=work:simon.perreault@viagenie.ca\r
GEO;TYPE=work:geo:46.772673,-71.282945\r
KEY;TYPE=work;VALUE=uri:\r
 http://www.viagenie.ca/simon.perreault/simon.asc\r
TZ:-0500\r
URL;TYPE=home:http://nomis80.org\r
END:VCARD\r
";

#[test]
fn rfc_6350_example_parses() {
    let card = VCard::parse(RFC_6350_EXAMPLE).unwrap();

    assert_eq!(*card.version(), Version::V4);
    assert_eq!(card.formatted_name().as_deref(), Some("Simon Perreault"));

    let name = card.structured_name().unwrap();
    assert_eq!(name.family, vec!["Perreault"]);
    assert_eq!(name.given, vec!["Simon"]);
    assert_eq!(name.suffixes, vec!["ing. jr", "M.Sc."]);

    let addresses = card.addresses();
    assert_eq!(addresses.len(), 1);
    assert_eq!(addresses[0].extended, "Suite D2-630");
    assert_eq!(addresses[0].street, "2875 Laurier");
    assert_eq!(addresses[0].locality, "Quebec");
    assert_eq!(addresses[0].postal_code, "G1V 2M2");
    assert_eq!(addresses[0].country, "Canada");
    assert_eq!(addresses[0].types, vec!["work"]);

    let phones: Vec<&Property> = card.properties_named("TEL").collect();
    assert_eq!(phones[0].raw_value(), "tel:+1-418-656-9254;ext=102");
    assert_eq!(phones[0].types(), vec!["work", "voice"]);
    assert_eq!(phones[0].pref(), Some(1));
    assert_eq!(phones[1].types().len(), 5);

    let langs: Vec<u32> = card
        .properties_named("LANG")
        .filter_map(Property::pref)
        .collect();
    assert_eq!(langs, vec![1, 2]);
    assert_eq!(
        card.first("KEY").map(Property::raw_value),
        Some("http://www.viagenie.ca/simon.perreault/simon.asc")
    );
    assert_eq!(card.email_addresses(), vec!["simon.perreault@viagenie.ca"]);
}

#[test]
fn vcard_round_trips() {
    let card = VCard::parse(RFC_6350_EXAMPLE).unwrap();
    let serialized = card.to_string();

    assert!(serialized.starts_with("BEGIN:VCARD\r\nVERSION:4.0\r\n"));
    assert!(serialized.ends_with("END:VCARD\r\n"));
    for physical in serialized.split("\r\n") {
        assert!(physical.len() <= 75);
    }
    assert_eq!(VCard::parse(&serialized).unwrap(), card);
}

#[test]
fn text_values_escape() {
    let card = VCard::new()
        .with(Property::text("NOTE", "a, b; c\\d\nsecond line"))
        .with(Property::list("CATEGORIES", &["Friends", "Work, Old"]))
        .with(Property::structured(
            "N",
            &[vec!["Doe"], vec!["Jo;hn"], vec![], vec![], vec![]],
        ));
    let reread = VCard::parse(&card.to_string()).unwrap();

    assert_eq!(
        reread.first("NOTE").map(Property::text_value).as_deref(),
        Some("a, b; c\\d\nsecond line")
    );
    assert_eq!(reread.categories(), vec!["Friends", "Work, Old"]);
    assert_eq!(
        reread.structured_name().map(|name| name.given),
        Some(vec!["Jo;hn".to_owned()])
    );
}

#[test]
fn parses_multiple_cards_rejects_malformed() {
    let two = format!("{RFC_6350_EXAMPLE}{RFC_6350_EXAMPLE}");
    assert_eq!(VCard::parse_all(&two).unwrap().len(), 2);
    assert!(matches!(VCard::parse(&two), Err(Error::SeveralVCards(2))));
    assert!(matches!(VCard::parse("NOTE:x\r\n"), Err(Error::NoVCard)));
    assert!(matches!(
        VCard::parse("BEGIN:VCARD\r\nFN:x\r\n"),
        Err(Error::UnbalancedVCard)
    ));
    assert!(matches!(
        VCard::parse("FN:x\r\nEND:VCARD\r\n"),
        Err(Error::UnbalancedVCard)
    ));
}

#[test]
fn vcard_3_upgrades_to_4() {
    let text = "BEGIN:VCARD\r
VERSION:3.0\r
FN:Old Style\r
TEL;TYPE=WORK,PREF:+1 555\r
EMAIL;TYPE=INTERNET;TYPE=PREF:old@example.com\r
PHOTO;ENCODING=b;TYPE=JPEG:/9j/4AAQ\r
 SkZJRg\r
KEY;ENCODING=B;TYPE=PGP:mQENBF\r
NOTE;CHARSET=UTF-8:hello\r
END:VCARD\r
";
    let cards = parse_vcf(text).unwrap();
    let card = &cards[0];

    assert_eq!(*card.version(), Version::V4);
    let phone = card.first("TEL").unwrap();
    assert_eq!(phone.pref(), Some(1));
    assert_eq!(phone.types(), vec!["work"]);
    assert_eq!(card.first("EMAIL").and_then(Property::pref), Some(1));
    assert_eq!(
        card.first("PHOTO").map(Property::raw_value),
        Some("data:image/jpeg;base64,/9j/4AAQSkZJRg")
    );
    assert_eq!(
        card.first("KEY").map(Property::raw_value),
        Some("data:application/pgp-keys;base64,mQENBF")
    );
    assert!(
        card.first("NOTE")
            .and_then(|note| note.parameter("CHARSET"))
            .is_none()
    );
}

#[test]
fn save_groups_emails_and_sets_uid() {
    let card = VCard::new()
        .with(Property::structured(
            "N",
            &[vec!["Doe"], vec!["Jane"], vec![], vec![], vec![]],
        ))
        .with(Property::text("EMAIL", "jane@example.com").with_pref(5))
        .with(Property::text("EMAIL", "doe@example.com").with_pref(2))
        .with(Property::list("CATEGORIES", &["A", "B"]))
        .with(Property::text("NOTE", "  "));
    let prepared = prepare_for_saving(&card);

    assert_eq!(prepared.formatted_name().as_deref(), Some("Jane Doe"));
    assert!(
        prepared
            .uid()
            .is_some_and(|uid| uid.starts_with("proton-web-"))
    );
    assert!(prepared.first("NOTE").is_none());
    assert_eq!(prepared.properties_named("CATEGORIES").count(), 2);

    let emails: Vec<(String, Option<u32>, Option<String>)> = prepared
        .emails()
        .map(|email| {
            (
                email.text_value(),
                email.pref(),
                email.group().map(str::to_owned),
            )
        })
        .collect();
    assert_eq!(
        emails,
        vec![
            (
                "doe@example.com".to_owned(),
                Some(1),
                Some("item1".to_owned())
            ),
            (
                "jane@example.com".to_owned(),
                Some(2),
                Some("item2".to_owned())
            ),
        ]
    );
}

#[test]
fn merge_keeps_first_name() {
    let first = VCard::new()
        .with(Property::text("FN", "Jane Doe"))
        .with(Property::text("UID", "first"))
        .with(Property::text("EMAIL", "jane@example.com").with_group("item1"))
        .with(Property::text("TEL", "+1 111"));
    let second = VCard::new()
        .with(Property::text("FN", "J. Doe"))
        .with(Property::text("UID", "second"))
        .with(Property::text("EMAIL", "jd@example.com").with_group("item1"))
        .with(Property::raw("KEY", "data:application/pgp-keys;base64,AAAA").with_group("item1"))
        .with(Property::text("TEL", "+1 111 "))
        .with(Property::text("TEL", "+1 222"));
    let merged = merge_vcards(&[first, second]);

    assert_eq!(merged.formatted_name().as_deref(), Some("Jane Doe"));
    assert_eq!(merged.uid().as_deref(), Some("first"));
    assert_eq!(merged.properties_named("TEL").count(), 2);
    assert_eq!(merged.email_group("jane@example.com"), Some("item1"));
    let moved = merged.email_group("jd@example.com").unwrap();
    assert_ne!(moved, "item1");
    assert!(
        merged
            .properties_named("KEY")
            .any(|key| key.in_group(moved))
    );
}

#[test]
fn mergeable_contacts_match_names_or_emails() {
    let contact = |id: &str, name: &str, email: &str| -> Contact {
        serde_json::from_value(json!({
            "ID": id,
            "Name": name,
            "ContactEmails": [{"ID": format!("{id}-email"), "Email": email, "ContactID": id}]
        }))
        .unwrap()
    };
    let contacts = vec![
        contact("a", "Jane Doe", "jane@example.com"),
        contact("b", "jane doe", "other@example.com"),
        contact("c", "Somebody", "OTHER@example.com"),
        contact("d", "Unknown", "lonely@example.com"),
        contact("e", "Unknown", "single@example.com"),
    ];

    assert_eq!(
        find_duplicates(&contacts),
        vec![vec![
            ContactId::from("a"),
            ContactId::from("b"),
            ContactId::from("c")
        ]]
    );
}

#[test]
fn vcard_2_1_imports_encodings() {
    let text = "BEGIN:VCARD\r
VERSION:2.1\r
N;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:M=C3=BCller;J=C3=BCrgen;;;\r
FN;CHARSET=ISO-8859-1;ENCODING=QUOTED-PRINTABLE:J=FCrgen M=FCller\r
NOTE;ENCODING=QUOTED-PRINTABLE:First line, with a comma=0D=0A=\r
second line\r
TEL;WORK;VOICE;PREF:+49 30 1234\r
EMAIL;INTERNET:juergen@example.de\r
ADR;HOME;CHARSET=windows-1252;QUOTED-PRINTABLE:;;Stra=DFe 1;Berlin;;10115;Deutschland\r
PHOTO;JPEG;ENCODING=BASE64:\r
 /9j/4AAQ\r
 SkZJRg\r
\r
END:VCARD\r
";
    let card = &parse_vcf(text).unwrap()[0];

    assert_eq!(*card.version(), Version::V4);
    assert_eq!(card.formatted_name().as_deref(), Some("Jürgen Müller"));
    let name = card.structured_name().unwrap();
    assert_eq!(name.family, vec!["Müller"]);
    assert_eq!(name.given, vec!["Jürgen"]);
    assert_eq!(
        card.first("NOTE").map(Property::text_value).as_deref(),
        Some("First line, with a comma\nsecond line")
    );
    let phone = card.first("TEL").unwrap();
    assert_eq!(phone.types(), vec!["work", "voice"]);
    assert_eq!(phone.pref(), Some(1));
    assert_eq!(
        card.first("EMAIL").map(Property::types),
        Some(vec!["internet".to_owned()])
    );
    let address = &card.addresses()[0];
    assert_eq!(address.street, "Straße 1");
    assert_eq!(address.country, "Deutschland");
    assert_eq!(address.types, vec!["home"]);
    assert_eq!(
        card.first("PHOTO").map(Property::raw_value),
        Some("data:image/jpeg;base64,/9j/4AAQSkZJRg")
    );
    for property in card.properties() {
        assert!(
            property.parameter("CHARSET").is_none(),
            "{}",
            property.name()
        );
        assert!(
            property.parameter("ENCODING").is_none(),
            "{}",
            property.name()
        );
    }

    let serialized = VCard::parse(&card.to_string()).unwrap();
    assert_eq!(&serialized, card);
}
