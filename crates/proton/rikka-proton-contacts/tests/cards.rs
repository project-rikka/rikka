use rikka_proton_contacts::{
    CardProblem, CardType, ContactCard, Property, VCard, decrypt_cards, seal_vcard, split_vcard,
};
use rikka_proton_pgp::{KeyRing, Pgp};

use rikka_test_proton::new_key;

fn sample() -> VCard {
    VCard::new()
        .with(Property::text("FN", "Ada Lovelace"))
        .with(Property::text("UID", "proton-web-ada"))
        .with(Property::text("EMAIL", "ada@example.com").with_group("item1"))
        .with(Property::text("X-PM-ENCRYPT", "true").with_group("item1"))
        .with(Property::text("TEL", "+44 20 7946 0000"))
        .with(Property::text("NOTE", "Analytical engine, 1843"))
        .with(Property::text("CATEGORIES", "Pioneers"))
}

#[test]
fn fields_split_into_cards() {
    let split = split_vcard(&sample());
    let names = |card: &VCard| -> Vec<String> {
        card.properties()
            .iter()
            .map(|property| property.name().to_owned())
            .collect()
    };

    assert_eq!(
        names(&split.signed),
        vec!["FN", "UID", "EMAIL", "X-PM-ENCRYPT"]
    );
    assert_eq!(names(&split.encrypted), vec!["TEL", "NOTE"]);
    assert_eq!(names(&split.clear), vec!["CATEGORIES"]);

    let without_categories = {
        let mut card = sample();
        card.remove_named("CATEGORIES");
        split_vcard(&card)
    };
    assert!(without_categories.clear.properties().is_empty());
}

#[test]
fn sealed_cards_open_verified() {
    let pgp = Pgp::new();
    let key = new_key(&pgp, "user@proton.test");
    let cards = seal_vcard(&pgp, &sample(), &key).unwrap();

    let kinds: Vec<CardType> = cards.iter().map(ContactCard::kind).collect();
    assert_eq!(
        kinds,
        vec![
            CardType::EncryptedAndSigned,
            CardType::Signed,
            CardType::Clear
        ]
    );
    assert!(cards[0].data().starts_with("-----BEGIN PGP MESSAGE-----"));
    assert!(!cards[0].data().contains("Analytical"));
    assert!(cards[1].data().contains("PREF=1:Ada Lovelace"));
    assert!(cards[1].data().contains("item1.EMAIL"));
    assert!(!cards[1].data().contains("TEL"));
    assert!(cards[2].signature().is_none());

    let ring = KeyRing::new().with_private(key);
    let decrypted = decrypt_cards(&pgp, &cards, &ring);
    assert!(decrypted.is_verified());
    assert!(decrypted.problems().is_empty());
    let card = decrypted.vcard();
    assert_eq!(card.formatted_name().as_deref(), Some("Ada Lovelace"));
    assert_eq!(card.uid().as_deref(), Some("proton-web-ada"));
    assert_eq!(
        card.first("NOTE").map(Property::text_value).as_deref(),
        Some("Analytical engine, 1843")
    );
    assert_eq!(card.categories(), vec!["Pioneers"]);
    assert_eq!(card.email_group("ada@example.com"), Some("item1"));
}

#[test]
fn tampered_cards_report_unverified() {
    let pgp = Pgp::new();
    let key = new_key(&pgp, "user@proton.test");
    let cards = seal_vcard(&pgp, &sample(), &key).unwrap();
    let tampered: Vec<ContactCard> = cards
        .iter()
        .map(|card| match card.kind() {
            CardType::Signed => ContactCard::new(
                CardType::Signed,
                card.data().replace("Ada Lovelace", "Mallory"),
                card.signature().map(str::to_owned),
            ),
            _ => card.clone(),
        })
        .collect();

    let decrypted = decrypt_cards(&pgp, &tampered, &KeyRing::new().with_private(key));
    assert!(!decrypted.is_verified());
    assert!(matches!(
        decrypted.problems(),
        [CardProblem::NotVerified {
            card: CardType::Signed,
            ..
        }]
    ));
    assert_eq!(
        decrypted.vcard().formatted_name().as_deref(),
        Some("Mallory")
    );
}

#[test]
fn foreign_keys_cannot_decrypt_card() {
    let pgp = Pgp::new();
    let key = new_key(&pgp, "user@proton.test");
    let stranger = new_key(&pgp, "stranger@proton.test");
    let cards = seal_vcard(&pgp, &sample(), &key).unwrap();

    let decrypted = decrypt_cards(&pgp, &cards, &KeyRing::new().with_private(stranger));
    assert!(!decrypted.is_verified());
    assert!(decrypted.problems().iter().any(|problem| matches!(
        problem,
        CardProblem::NotDecrypted {
            card: CardType::EncryptedAndSigned,
            ..
        }
    )));
    assert!(decrypted.vcard().first("NOTE").is_none());
    assert!(!decrypted.is_complete());

    let unknown = [ContactCard::new(CardType::Other(9), "x", None)];
    assert!(matches!(
        decrypt_cards(&pgp, &unknown, &KeyRing::new()).problems(),
        [CardProblem::UnknownType { kind: 9 }]
    ));
}
