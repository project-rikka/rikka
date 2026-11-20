use rikka_llm::ModelInfo;
use rikka_llm_providers::{Backend, Credential, Error, Renewal};
use serde_json::json;

fn backend(id: &str) -> Backend {
    Backend::from_id(id).expect("backend should be registered")
}

#[test]
fn api_key_matches_keyed_backends() {
    let credential = Credential::keyed("notarealkey");
    assert!(backend("anthropic").holds(&credential));
    assert!(backend("openai").holds(&credential));
    assert!(!backend("claude-code").holds(&credential));
}

#[test]
fn mismatched_credential_is_ignored() {
    let credential = Credential::keyed("notarealkey");
    let model = ModelInfo::new("gpt-6.1-sol", "GPT-6.1 Sol", 272_000, 0);
    assert!(matches!(
        backend("codex").connect(Some(&credential), &model, &Renewal::none()),
        Err(Error::SignedOut(signed_out)) if signed_out == backend("codex")
    ));
}

#[tokio::test]
async fn listing_needs_the_account_signed_in() {
    let listed: Vec<Backend> = Backend::all()
        .filter(|backend| backend.is_listed())
        .collect();
    assert_eq!(listed.len(), Backend::all().len() - 1);
    assert!(!backend("openrouter").is_listed());

    for backend in listed {
        let refused = backend.listed(None, &Renewal::none()).await;
        assert!(
            matches!(
                refused,
                Err(Error::Keyless(named) | Error::SignedOut(named)) if named == backend
            ),
            "{backend:?} listed without an account: {refused:?}"
        );
    }
}

#[test]
fn credentials_round_trip() {
    for raw in [
        json!({"kind": "key", "key": "notarealkey"}),
        json!({"kind": "claude-code", "tokens": {"access_token": "notarealtoken", "refresh_token": "notarealtoken"}}),
    ] {
        let credential: Credential =
            serde_json::from_value(raw.clone()).expect("credential should deserialize");
        let rewritten = serde_json::to_value(&credential).expect("credential should serialize");
        assert_eq!(rewritten, raw);
    }
}

#[test]
fn unusable_credential_is_rejected_on_read() {
    for raw in [
        json!({"kind": "key"}),
        json!({"kind": "claude-code", "key": "notarealkey"}),
        json!({"kind": "no-such-backend", "tokens": {}}),
        json!({"kind": "claude-code", "tokens": {"email": "somebody@example.com"}}),
    ] {
        assert!(serde_json::from_value::<Credential>(raw).is_err());
    }
}
