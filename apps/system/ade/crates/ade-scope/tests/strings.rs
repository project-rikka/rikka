use ade_scope::strings::{is_valid_name, truncate};

#[test]
fn valid_names_are_kebab_case() {
    assert!(is_valid_name("code-review"));
    assert!(is_valid_name("pdf2md"));

    assert!(!is_valid_name(""));
    assert!(!is_valid_name("Code-Review"));
    assert!(!is_valid_name("code--review"));
    assert!(!is_valid_name("-review"));
    assert!(!is_valid_name("review-"));
    assert!(!is_valid_name("code review"));
}

#[test]
fn truncate_cuts_at_char_boundary() {
    let cut = truncate(&"あ".repeat(10), 4).expect("it runs long");

    assert_eq!(cut.chars().count(), 4);
    assert!(cut.ends_with('…'));
    assert!(cut.starts_with("あああ"));
}
