use rikka_syntax::{Kind, Language, Lexicon, Parsed, Suggestion};
use rikka_text_document::Content;
use rikka_text_document::Rope;

fn words(suggestions: &[Suggestion]) -> Vec<&str> {
    suggestions.iter().map(Suggestion::word).collect()
}

#[test]
fn completion_matches_word_and_camel_starts() {
    let rope = Rope::from("maximum makeRequest request value\nreq");
    let lexicon = Lexicon::from_text(&rope, rope.len(), None);

    assert_eq!(
        words(&lexicon.suggest("req", 10)),
        ["request", "makeRequest"]
    );
    assert!(lexicon.suggest("x", 10).is_empty());
}

#[test]
fn lexicon_classifies_words_and_offers_keywords() {
    let content = Content::new(
        "struct Walker { depth: usize }\nfn walk_tree(walker: Walker) {}\nfn main() { wa ma }",
    );
    let mut rust =
        Parsed::new(Language::named("rust").expect("rust grammar missing")).expect("rust queries");
    let at = content.text().find("wa ").expect("partial word") + 2;

    let offered = rust.lexicon(&content, at).suggest("wa", 10);
    let named: Vec<(&str, Kind)> = offered
        .iter()
        .map(|suggestion| (suggestion.word(), suggestion.kind()))
        .collect();
    assert!(named.contains(&("walk_tree", Kind::Function)));
    assert!(named.contains(&("Walker", Kind::Type)));
    assert!(named.contains(&("walker", Kind::Variable)));

    let at = content.text().find("ma }").expect("partial keyword") + 2;
    let offered = rust.lexicon(&content, at).suggest("ma", 10);
    assert!(
        offered
            .iter()
            .any(|suggestion| suggestion.word() == "match" && suggestion.kind() == Kind::Keyword)
    );
}
