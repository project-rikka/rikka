use rikka_llm::{Effort, ModelInfo};
use rikka_llm_accounts::{
    Endpoint, EndpointCredentials, Header, ModelOptions, Row, Selection, Wire,
};

use crate::support::Desk;

const KEY: &str = "notarealkey";
const SECRET: &str = "notarealtoken";

fn credentials() -> EndpointCredentials {
    let header = Header::new("cf-access-client-secret", SECRET).expect("header should be valid");
    EndpointCredentials::new(KEY, vec![header])
}

fn model(id: &str) -> ModelInfo {
    ModelInfo::new(id, id, 40_960, 8_192)
        .with_vision(true)
        .with_efforts(vec![Effort::Low, Effort::High])
}

#[test]
fn endpoints_round_trip_with_secrets_in_the_keyring() {
    let desk = Desk::fresh();
    let Some(mut llm) = desk.load() else {
        return;
    };
    let ollama = llm.add_endpoint(
        "Ollama",
        Endpoint::new("http://127.0.0.1:11434").expect("address should be accepted"),
        credentials(),
    );
    let gateway = llm.add_endpoint(
        "Gateway",
        Endpoint::new("https://gateway.example/openai/v1")
            .expect("address should be accepted")
            .with_wire(Wire::Responses),
        EndpointCredentials::default(),
    );
    llm.add_model(ollama, model("qwen3:8b"));
    llm.set_model(ollama, "qwen3:8b");
    llm.add_model(gateway, model("gpt-oss-120b"));
    llm.select_account(ollama);

    let read = desk.load().expect("the keyring kept running");
    let kept = read.account(ollama).expect("the endpoint should be kept");
    assert_eq!(kept.label(), "Ollama");
    let endpoint = kept.endpoint().expect("the account is an endpoint");
    assert_eq!(endpoint.base(), "http://127.0.0.1:11434/v1");
    assert_eq!(endpoint.wire(), Wire::Completions);
    assert_eq!(kept.endpoint_credentials(), Some(&credentials()));
    let info = kept.model_info().expect("the chosen model should be kept");
    assert_eq!(info.context(), 40_960);
    assert!(info.vision());
    assert_eq!(info.efforts(), [Effort::Low, Effort::High]);
    assert_eq!(
        read.account(gateway)
            .and_then(|account| account.endpoint())
            .map(Endpoint::wire),
        Some(Wire::Responses)
    );
    assert_eq!(
        read.selected_account().map(|account| account.id()),
        Some(ollama)
    );

    let settings = desk.settings_text();
    assert!(settings.contains("127.0.0.1:11434"));
    for secret in [KEY, SECRET] {
        assert!(!settings.contains(secret), "{secret} reached the settings");
    }
}

#[test]
fn endpoint_models_are_offered_like_any_other() {
    let desk = Desk::fresh();
    let Some(mut llm) = desk.load() else {
        return;
    };
    let first = llm.add_endpoint(
        "Ollama",
        Endpoint::new("http://127.0.0.1:11434").expect("address should be accepted"),
        EndpointCredentials::default(),
    );
    let second = llm.add_endpoint(
        "vLLM",
        Endpoint::new("http://127.0.0.1:8000").expect("address should be accepted"),
        credentials(),
    );
    llm.add_model(first, model("qwen3:8b"));
    llm.add_model(second, model("Qwen/Qwen3-32B"));

    let options = ModelOptions::new(&llm, None);
    let headings: Vec<&str> = options
        .rows()
        .iter()
        .filter_map(|row| match row {
            Row::Account(at) => Some(options.options()[*at].account()),
            Row::Model(_) | Row::Separator | Row::Levels => None,
        })
        .collect();
    assert_eq!(headings, ["Ollama", "vLLM"]);

    let selection = Selection::new(second, "Qwen/Qwen3-32B", Effort::High);
    assert_eq!(
        selection
            .connect(&llm)
            .expect("the endpoint model should connect")
            .name(),
        "Qwen/Qwen3-32B"
    );
    let named = llm
        .roster()
        .connect_named("qwen/qwen3 32b", None)
        .expect("the roster should find the endpoint model");
    assert_eq!(named.context_window(), 40_960);
    assert!(llm.roster().summary().contains("vLLM"));

    llm.remove_endpoint(second);
    let read = desk.load().expect("the keyring kept running");
    assert!(read.account(second).is_none());
    assert!(read.account(first).is_some());
    assert!(selection.connect(&read).is_err());
}
