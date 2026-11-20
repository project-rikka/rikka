use rikka_syntax::{Language, Parsed};
use rikka_text::TextSpan;
use rikka_theme::{Theme, Token};

#[test]
fn grammars_compile_their_queries() {
    for grammar in rikka_grammars::Grammar::all() {
        assert_eq!(
            Language::named(grammar.name()).map(Language::name),
            Some(grammar.name()),
            "the {} grammar is no language",
            grammar.name()
        );
    }
    for language in Language::all() {
        assert!(
            Parsed::new(language).is_some(),
            "{} queries fail to compile",
            language.name()
        );
    }
}
#[test]
fn shipped_slang_grammar_is_the_locked_crate() {
    let shipped = rikka_grammars::Grammar::named("slang")
        .and_then(rikka_grammars::Grammar::language)
        .expect("the slang grammar loads");
    let locked = tree_sitter::Language::new(tree_sitter_slang::LANGUAGE_SLANG);
    assert_eq!(shipped.node_kind_count(), locked.node_kind_count());
    assert_eq!(shipped.abi_version(), locked.abi_version());
}

#[test]
fn language_lookup_by_name_and_path() {
    assert_eq!(Language::named("Rust"), Language::named("rs"));
    assert_eq!(Language::named("c++"), Language::named("cpp"));
    assert!(Language::named("klingon").is_none());
    assert!(Language::named("").is_none());

    assert_eq!(
        Language::from_path("crates/ui/rikka-ui/BUILD.bazel").map(Language::name),
        Some("starlark")
    );
    assert_eq!(
        Language::from_path("/etc/nixos/flake.nix").map(Language::name),
        Some("nix")
    );
    assert_eq!(
        Language::from_path("shaders/rim.slang").map(Language::name),
        Some("slang")
    );
    assert_eq!(Language::from_path("notes.unknown"), None);
}

#[test]
fn injection_highlights_whole_fence() {
    let syntax = Theme::dark().syntax();
    let markdown = Language::named("markdown").expect("markdown grammar missing");
    let spans = markdown.spans("```rust\n// a note\nfn main() {}\n```\n", syntax);

    let painted = |token: Token, range: std::ops::Range<usize>| {
        spans
            .iter()
            .any(|span: &TextSpan| span.range == range && span.color == Some(syntax.color(token)))
    };
    assert!(
        painted(Token::Comment, 8..17),
        "fenced note not highlighted"
    );
    assert!(
        painted(Token::Keyword, 18..20),
        "fenced rust not highlighted"
    );
}
