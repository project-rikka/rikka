const VECTORS: &str = "crates/foundation/rikka-public-suffix/tests/test_psl.txt";
const CALL: &str = "checkPublicSuffix(";

fn argument(text: &str) -> Option<String> {
    let text = text.trim();
    (text != "null").then(|| {
        text.strip_prefix('\'')
            .and_then(|text| text.strip_suffix('\''))
            .expect("quoted argument")
            .to_owned()
    })
}

#[test]
fn registrable_domains_match_official_vectors() {
    let vectors = std::fs::read_to_string(VECTORS).expect("test vectors");
    let mut checked = 0;
    for line in vectors.lines().map(str::trim) {
        let Some(call) = line
            .strip_prefix(CALL)
            .and_then(|call| call.strip_suffix(");"))
        else {
            continue;
        };
        let (input, expected) = call.split_once(", ").expect("two arguments");
        let Some(input) = argument(input) else {
            continue;
        };
        let expected = argument(expected);
        let found = rikka_public_suffix::registrable_domain(&input);
        assert!(
            match (found, expected.as_deref()) {
                (Some(found), Some(expected)) => found.eq_ignore_ascii_case(expected),
                (None, None) => true,
                _ => false,
            },
            "{input}: expected {expected:?}, found {found:?}"
        );
        checked += 1;
    }
    assert!(checked > 50, "only {checked} vectors read");
}
