use rikka_mime::{matches_glob, matches_glob_folding_case};

#[test]
fn matches_shell_wildcards() {
    let cases = [
        ("*.tar.*", "a.tar.gz", true),
        ("a?c", "abc", true),
        ("a?c", "ac", false),
        ("[abc]x", "bx", true),
        ("[!abc]x", "bx", false),
        ("[a-f]x", "dx", true),
        ("*", "anything at all", true),
        ("", "", true),
        ("", "x", false),
        ("*a*b", "xxaxxb", true),
        ("*a*b", "xxaxx", false),
        ("core.[0-9]*", "core.1234", true),
        ("core.[0-9]*", "core.abc", false),
        ("[]]", "]", true),
        ("[abc", "[abc", true),
        ("*.png", "CAT.PNG", false),
        ("*.png", "cat.png.bak", false),
        ("image/*", "image/png", true),
        ("image/*", "video/mp4", false),
        ("写真*", "写真とねこ", true),
        ("?真", "写真", true),
    ];

    for (pattern, subject, matched) in cases {
        assert_eq!(
            matches_glob(pattern, subject),
            matched,
            "{pattern} on {subject}"
        );
    }
}

#[test]
fn case_folding_is_ascii_only() {
    assert!(matches_glob_folding_case("*.png", "CAT.PNG"));
    assert!(matches_glob_folding_case("*.tar.gz", "Archive.TAR.GZ"));
    assert!(matches_glob_folding_case("[A-F]x", "dx"));
    assert!(matches_glob_folding_case("*.png", "写真.png"));
}

#[test]
fn stars_match_in_linear_time() {
    let pattern = "*a*a*a*a*a*a*a*a*a*a*a*a*b";
    let subject = "a".repeat(64);

    assert!(!matches_glob(pattern, &subject));
}
