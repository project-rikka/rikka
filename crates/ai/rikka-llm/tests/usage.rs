use rikka_llm::wire::endpoint_under;
use rikka_llm::{Price, Usage};

#[test]
fn token_kinds_use_their_own_rate() {
    let usage = Usage {
        input: 1_000_000,
        output: 1_000_000,
        cache_read: 1_000_000,
        cache_write: 1_000_000,
        reasoning: 500_000,
    };

    assert!((Price::new(5.0, 25.0).cost(usage) - (5.0 + 25.0 + 0.5 + 6.25)).abs() < 1e-9);
}

#[test]
fn path_joins_base_regardless_of_slash() {
    for base in ["https://example.test/v1", "https://example.test/v1/"] {
        assert_eq!(
            endpoint_under(base, "/chat/completions")
                .expect("base should be a URL")
                .as_str(),
            "https://example.test/v1/chat/completions"
        );
    }
}
