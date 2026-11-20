use rikka_emoji::{Catalog, Tone};

fn english() -> Catalog {
    Catalog::read(&"en".parse().expect("english is a locale"))
}

#[test]
fn applies_requested_skin_tone() {
    let catalog = english();
    let wave = catalog
        .get(catalog.position("👋").expect("waving is listed"))
        .expect("it was just found");

    assert!(wave.supports_tone());
    assert_eq!(wave.with_tone(Tone::Plain), "👋");
    assert_eq!(wave.with_tone(Tone::Light), "👋🏻");
    assert_eq!(wave.with_tone(Tone::Dark), "👋🏿");

    let flag = catalog
        .get(catalog.position("🏁").expect("the flag is listed"))
        .expect("it was just found");
    assert!(!flag.supports_tone());
    assert_eq!(flag.with_tone(Tone::Dark), "🏁", "a flag has no skin");
}

#[test]
fn search_ranks_name_and_requires_all_words() {
    let catalog = english();
    let found = catalog.search("cat");
    let first = catalog.get(found[0]).expect("something was found");
    assert_eq!(first.name, "cat");
    assert!(found.len() > 1, "the cats only described as cats follow it");

    assert!(!catalog.search("crying cat").is_empty());
    assert!(catalog.search("cat helicopter").is_empty());
    assert!(catalog.search("   ").is_empty());
}

#[test]
fn japanese_catalog_searches_all_languages() {
    let catalog = Catalog::read(&"ja".parse().expect("japanese is a locale"));
    let cat = catalog.position("🐱").expect("the cat face is listed");
    let found = catalog.get(cat).expect("just found");
    assert_eq!(found.name, "ネコの顔");
    assert_eq!(found.code, "cat_face");

    assert!(catalog.search("ねこ").contains(&cat));
    assert!(catalog.search("cat").contains(&cat));
    assert!(catalog.search("cat_face").contains(&cat));
}
