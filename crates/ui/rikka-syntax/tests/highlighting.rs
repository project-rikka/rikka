use std::time::Duration;

use rikka_syntax::{Language, Parsed};
use rikka_text_document::Content;
use rikka_theme::{Theme, Token};

const PLENTY: Duration = Duration::from_secs(5);

fn parsed(name: &str) -> Parsed {
    let language = Language::named(name).expect("grammar missing");
    Parsed::new(language).expect("queries fail to compile")
}

fn token(parsed: &mut Parsed, content: &Content, at: usize) -> Option<Token> {
    assert!(parsed.read(content, PLENTY));
    parsed.token_at(content, at + 1)
}

#[test]
fn document_highlights_by_capture() {
    let content = Content::new("fn main() { let greeting = \"hello\"; } // note");
    let mut rust = parsed("rust");

    assert_eq!(token(&mut rust, &content, 0), Some(Token::Keyword));
    assert_eq!(token(&mut rust, &content, 3), Some(Token::Function));
    assert_eq!(token(&mut rust, &content, 28), Some(Token::String));
    assert_eq!(rust.token_at(&content, content.len()), Some(Token::Comment));

    let other = Content::new("const WIDTH: u32 = 4;");
    assert_eq!(
        token(&mut rust, &other, 13),
        Some(Token::Type),
        "new document not scrolled to top"
    );
}

#[test]
fn edits_and_undo_preserve_highlights() {
    let mut content = Content::new("fn main() {}");
    let mut rust = parsed("rust");
    assert!(rust.read(&content, PLENTY));

    content.place_caret(0, false);
    content.insert_str("// a note\n");
    assert_eq!(token(&mut rust, &content, 0), Some(Token::Comment));
    assert_eq!(token(&mut rust, &content, 10), Some(Token::Keyword));

    content.undo();
    assert_eq!(token(&mut rust, &content, 0), Some(Token::Keyword));

    content.set_text("const WIDTH: u32 = 4;");
    assert_eq!(token(&mut rust, &content, 13), Some(Token::Type));
}

#[test]
fn wide_indent_change_reparses() {
    let lines = 400;
    let mut content = Content::new(&"fn main() {}\n".repeat(lines));
    let mut rust = parsed("rust");
    assert!(rust.read(&content, PLENTY));

    content.select_all();
    content.indent("    ");

    let last = (lines - 1) * "    fn main() {}\n".len();
    assert_eq!(token(&mut rust, &content, last + 4), Some(Token::Keyword));
}

#[test]
fn timed_out_parse_resumes() {
    let content = Content::new(&"fn main() { let x = 1; }\n".repeat(20_000));
    let mut rust = parsed("rust");

    assert!(!rust.read(&content, Duration::ZERO));
    assert_eq!(token(&mut rust, &content, 0), Some(Token::Keyword));
}

#[test]
fn edit_keeps_untouched_injection_highlights() {
    let mut markdown = parsed("markdown");
    let mut content = Content::new("```rust\nfn one() {}\n```\n\n```rust\nfn two() {}\n```\n");
    assert!(markdown.read(&content, PLENTY));
    let colors = Theme::dark().syntax();
    assert!(
        !markdown
            .spans(&content, 0..content.len(), colors)
            .is_empty()
    );

    content.place_caret(8, false);
    content.insert_str("// a note\n");
    assert!(markdown.read(&content, PLENTY));
    assert!(
        !markdown
            .spans(&content, 0..content.len(), colors)
            .is_empty()
    );
    let text = content.text();
    let one = text.find("fn one").expect("first block missing");
    let two = text.find("fn two").expect("second block missing");
    assert_eq!(token(&mut markdown, &content, one), Some(Token::Keyword));
    assert_eq!(token(&mut markdown, &content, two), Some(Token::Keyword));
    assert_eq!(token(&mut markdown, &content, 8), Some(Token::Comment));
}
