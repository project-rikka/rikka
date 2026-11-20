use rikka_chat::{DraftChange, Offers, invoked, mention_at, mentioned};
use rikka_ui::TextDocument;

#[test]
fn sent_draft_lists_mentions_once() {
    for (draft, paths) in [
        (
            "compare @a/one.rs with @\"b/two three.rs\", and @a/one.rs again",
            vec!["a/one.rs", "b/two three.rs"],
        ),
        ("@src/main.rs, and more", vec!["src/main.rs"]),
        ("see @\"odd @ name.md\"", vec!["odd @ name.md"]),
        (
            "@ドキュメント/はじめに.md",
            vec!["ドキュメント/はじめに.md"],
        ),
        ("someone@example.com and a bare @ sign", vec![]),
    ] {
        assert_eq!(mentioned(draft), paths, "{draft}");
    }
}

#[test]
fn mention_at_caret_is_current_path() {
    let quoted = mention_at("see @\"my notes/one two.md\"", 26).expect("mention should be found");
    assert_eq!(quoted.query(), "my notes/one two.md");
    assert_eq!(quoted.at(), 4);

    let unclosed = mention_at("see @\"my not", 12).expect("mention should be found");
    assert_eq!(unclosed.query(), "my not");
    assert_eq!(unclosed.end(), 12);

    assert_eq!(mention_at("@src/main.rs and more", 20), None);
    assert_eq!(mention_at("name@example.com", 16), None);
}

#[test]
fn slash_draft_invokes_command() {
    for (draft, name, arguments) in [
        (
            "/review src/main.rs and be quick",
            "review",
            "src/main.rs and be quick",
        ),
        ("/commit", "commit", ""),
        (
            "/review\nthe composer\nand the picker",
            "review",
            "the composer\nand the picker",
        ),
        ("/レビュー src/", "レビュー", "src/"),
        ("/github:review 12 now", "github:review", "12 now"),
    ] {
        let found = invoked(draft).expect("draft should invoke a command");
        assert_eq!(
            (found.name(), found.arguments()),
            (name, arguments),
            "{draft}"
        );
    }
    for draft in ["/", "/ review", "review", "look at /review"] {
        assert!(invoked(draft).is_none(), "{draft}");
    }
}

#[test]
fn caret_in_arguments_names_argument() {
    for (draft, caret, given, typed) in [
        ("/echo:greet Ada \"very wa", None, vec!["Ada"], "very wa"),
        ("/echo:greet Ada ", None, vec!["Ada"], ""),
        ("/echo:greet 'one two' th", None, vec!["one two"], "th"),
        ("/echo:greet Ada more", Some(13), vec![], "A"),
    ] {
        let document = TextDocument::new(draft);
        document.place_caret(caret.unwrap_or(draft.len()), false);
        let mut offers = Offers::default();
        let DraftChange::Argument(request) = offers.typed(&document) else {
            panic!("{draft} should be typing an argument");
        };
        let argument = offers
            .pending_argument(request, &document)
            .expect("argument should be pending");
        assert_eq!(argument.command(), "echo:greet", "{draft}");
        assert_eq!(argument.given(), given, "{draft}");
        assert_eq!(argument.typed(), typed, "{draft}");
    }

    let named = TextDocument::new("/echo:gr");
    named.place_caret(8, false);
    assert_eq!(
        Offers::default().typed(&named),
        DraftChange::Command("echo:gr".to_owned())
    );
}
