use std::fmt::Debug;

use rikka_llm::{Model, ModelInfo};
use rikka_llm_antigravity::AntigravityCredentials;
use rikka_llm_claude_code::ClaudeCodeCredentials;
use rikka_llm_codex::CodexCredentials;
use rikka_llm_providers::{
    Anthropic, Backend, Credential, Endpoint, EndpointCredentials, Header, OpenAi, OpenRouter,
    Renewal,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;

const SECRET: &str = "notarealtoken";

fn assert_hidden<T: Serialize + DeserializeOwned + Debug>() {
    let credentials: T = serde_json::from_value(json!({
        "id_token": SECRET,
        "access_token": SECRET,
        "refresh_token": SECRET,
        "key": SECRET,
        "api_key": SECRET,
        "token_endpoint": "https://auth.example.com/oauth/token",
        "email": "somebody@example.com",
    }))
    .expect("credentials should deserialize");

    assert!(
        !format!("{credentials:?}").contains(SECRET),
        "a credential reached a log"
    );
    assert!(
        serde_json::to_string(&credentials)
            .expect("credentials should serialize")
            .contains(SECRET),
        "a credential was not stored"
    );
}

fn assert_unspoken(model: &Model) {
    let logged = format!("{model:?}");
    assert!(!logged.contains(SECRET), "a credential reached a log");
}

fn endpoint_credentials() -> EndpointCredentials {
    let header = Header::new("cf-access-client-secret", SECRET).expect("header should be valid");
    EndpointCredentials::new(SECRET, vec![header])
}

#[test]
fn credentials_are_stored_but_redacted() {
    assert_hidden::<AntigravityCredentials>();
    assert_hidden::<ClaudeCodeCredentials>();
    assert_hidden::<CodexCredentials>();
    assert_hidden::<EndpointCredentials>();
}

#[test]
fn api_key_is_redacted() {
    let builders = [
        format!("{:?}", Anthropic::new(SECRET).model("claude-opus-5")),
        format!("{:?}", OpenAi::new(SECRET).model("gpt-5.2")),
        format!(
            "{:?}",
            OpenRouter::new(SECRET).model("anthropic/claude-opus-5")
        ),
        format!("{:?}", endpoint_credentials()),
    ];
    for logged in builders {
        assert!(!logged.contains(SECRET), "{logged}");
    }

    let models = [
        Anthropic::new(SECRET).model("claude-opus-5").build(),
        OpenAi::new(SECRET).model("gpt-5.2").build(),
        OpenRouter::new(SECRET)
            .model("anthropic/claude-opus-5")
            .build(),
        Endpoint::new("https://example.test/v1")
            .and_then(|endpoint| endpoint.connect(&endpoint_credentials(), "a-model")),
    ];
    for model in models {
        assert_unspoken(&model.expect("model should be configured"));
    }
    assert!(!format!("{:?}", Credential::keyed(SECRET)).contains(SECRET));
}

#[test]
fn account_debug_hides_tokens() {
    let tokens = json!({
        "access_token": SECRET,
        "refresh_token": SECRET,
        "api_key": SECRET,
        "key": SECRET,
        "project_id": "projects/42",
        "token_endpoint": "https://auth.example.com/oauth/token",
        "email": "somebody@example.com",
    });
    let mut connected = 0;
    for backend in Backend::all().filter(|backend| backend.is_subscription()) {
        let credential: Credential =
            serde_json::from_value(json!({ "kind": backend.id(), "tokens": tokens }))
                .unwrap_or_else(|error| panic!("{backend:?} cannot read its tokens: {error}"));
        assert!(!format!("{credential:?}").contains(SECRET));

        let model = ModelInfo::new("a-model", "A model", 200_000, 64_000);
        if let Ok(model) = backend.connect(Some(&credential), &model, &Renewal::none()) {
            assert_unspoken(&model);
            connected += 1;
        }
    }
    assert!(connected > 0, "no account could be connected");
}
