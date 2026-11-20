use rikka_llm::Error;
use rikka_llm_providers::{Anthropic, Endpoint, EndpointCredentials, OpenAi, OpenRouter};

const KEY: &str = "notarealkey";

#[test]
fn invalid_header_fails_at_build() {
    for error in [
        Anthropic::new(KEY)
            .model("claude-opus-5")
            .header("a name with spaces", "value")
            .build()
            .expect_err("the name is not a header's"),
        OpenAi::new(KEY)
            .model("gpt-5.2")
            .header("x-note", "a value with a \n in it")
            .build()
            .expect_err("the value is not a header's"),
    ] {
        assert!(matches!(&error, Error::Rejected(what) if what == "a header is not valid"));
    }
}

#[test]
fn model_is_required() {
    for error in [
        Anthropic::new(KEY).build().expect_err("no model was named"),
        OpenAi::new(KEY).build().expect_err("no model was named"),
        OpenRouter::new(KEY)
            .build()
            .expect_err("no model was named"),
        Endpoint::new("https://example.test/v1")
            .and_then(|endpoint| endpoint.connect(&EndpointCredentials::default(), ""))
            .expect_err("no model was named"),
    ] {
        assert!(matches!(&error, Error::Rejected(what) if what == "no model was named"));
    }
}

#[test]
fn invalid_endpoint_fails_at_build() {
    for error in [
        Anthropic::new(KEY)
            .model("claude-opus-5")
            .endpoint("nowhere in particular")
            .build()
            .expect_err("the endpoint is not a URL"),
        Endpoint::new("nowhere in particular").expect_err("the base is not a URL"),
    ] {
        assert!(matches!(error, Error::InvalidEndpoint(_)));
    }
}
