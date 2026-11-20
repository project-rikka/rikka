use rikka_llm::{Effort, ModelInfo};
use rikka_llm_accounts::{Account, Credential, Error, Llm, Selection};

use crate::support::{Desk, backend};

const LISTED_AT: u64 = 1_800_000_000;

fn account(llm: &Llm, id: &str) -> Account {
    llm.account(backend(id).into())
        .cloned()
        .expect("every backend should have an account")
}

fn ids(models: &[ModelInfo]) -> Vec<&str> {
    models.iter().map(ModelInfo::id).collect()
}

fn listing(backends: &[(&str, &[(&str, bool)])]) -> String {
    let mut text = String::new();
    for (backend, models) in backends {
        text.push_str(&format!(
            "[[backends]]\nbackend = \"{backend}\"\nlisted = {LISTED_AT}\n\n"
        ));
        for (id, superseded) in *models {
            text.push_str(&format!(
                "[[backends.models]]\nid = \"{id}\"\nname = \"Named {id}\"\ncontext = 1000000\n\
                 output = 128000\nvision = true\nefforts = [\"low\", \"high\"]\n\
                 superseded = {superseded}\n\n"
            ));
        }
    }
    text
}

fn antigravity(token: &str) -> Credential {
    serde_json::from_value(serde_json::json!({
        "kind": "antigravity",
        "tokens": {
            "access_token": token,
            "refresh_token": token,
            "project_id": "projects/42"
        }
    }))
    .expect("credential should deserialize")
}

#[test]
fn setup_round_trips() {
    let desk = Desk::fresh();
    let Some(mut llm) = desk.load() else {
        return;
    };
    let credential: Credential = serde_json::from_str(
        r#"{"kind":"claude-code","tokens":{"access_token":"notarealtoken",
            "refresh_token":"notarealtoken","email":"somebody@example.com",
            "organization_name":"Example"}}"#,
    )
    .expect("credential should deserialize");
    let picked = serde_json::from_str(
        r#"{"id":"anthropic/claude-opus-5","name":"Claude Opus 5","context":1000000,
            "output":128000,"vision":true,"tools":true,"efforts":["low","high"],
            "price":{"input":5.0,"output":25.0,"cache_read":0.5,"cache_write":6.25}}"#,
    )
    .expect("picked model should parse");

    llm.set_credential(backend("claude-code"), credential);
    desk.write_settings(&listing(&[("claude-code", &[("claude-opus-5-5", false)])]));
    let mut llm = desk.load().expect("the keyring kept running");
    llm.set_model(backend("claude-code").into(), "claude-opus-5-5");
    llm.set_api_key(backend("openrouter").into(), "notarealkey");
    llm.add_model(backend("openrouter").into(), picked);
    llm.set_model(backend("openrouter").into(), "anthropic/claude-opus-5");
    llm.select_account(backend("openrouter").into());
    llm.set_effort(Effort::Low);
    llm.set_language("Japanese");

    let read = desk.load().expect("the keyring kept running");
    let claude = account(&read, "claude-code");
    assert_eq!(claude.model(), "claude-opus-5-5");
    assert_eq!(ids(claude.models()), ["claude-opus-5-5"]);
    assert_eq!(claude.identity(), "somebody@example.com · Example");
    let routed = read.selected_account().expect("account should be selected");
    assert_eq!(routed.backend(), Some(backend("openrouter")));
    assert_eq!(routed.stored_key(), "notarealkey");
    let model = routed.model_info().expect("account should offer its model");
    assert_eq!(model.name(), "Claude Opus 5");
    assert_eq!(model.price().map(|price| price.input), Some(5.0));
    assert_eq!(read.effort(), Effort::Low);
    assert_eq!(read.reply_language(), "Japanese");
    assert_eq!(
        read.accounts().len(),
        rikka_llm_accounts::Backend::all().len()
    );
    let settings = desk.settings_text();
    for secret in ["notarealkey", "notarealtoken"] {
        assert!(!settings.contains(secret), "{secret} reached the settings");
    }
}

#[test]
fn selection_survives_default_change() {
    let desk = Desk::fresh();
    let Some(mut llm) = desk.load() else {
        return;
    };
    for name in ["anthropic", "openai"] {
        llm.set_api_key(backend(name).into(), "notarealkey");
    }
    desk.write_settings(&listing(&[
        (
            "anthropic",
            &[("claude-opus-5-5", false), ("claude-sonnet-5-5", false)],
        ),
        ("openai", &[("gpt-6.1-sol", false)]),
    ]));
    let mut llm = desk.load().expect("the keyring kept running");
    for name in ["anthropic", "openai"] {
        let model = account(&llm, name).models()[0].id().to_owned();
        llm.set_model(backend(name).into(), &model);
    }
    llm.select_account(backend("anthropic").into());
    llm.set_effort(Effort::High);

    let first = llm.selection().expect("selection should exist");
    llm.select_account(backend("openai").into());
    llm.set_effort(Effort::Low);
    let second = llm.selection().expect("second selection should exist");
    let replacement = account(&llm, "anthropic").models()[1].id().to_owned();
    llm.set_model(backend("anthropic").into(), &replacement);

    assert_eq!(first.account(), backend("anthropic").into());
    assert_eq!(first.effort(), Effort::High);
    assert_eq!(second.account(), backend("openai").into());
    assert_eq!(second.effort(), Effort::Low);
    assert_eq!(llm.selection().as_ref(), Some(&second));
    for selection in [&first, &second] {
        assert_eq!(
            selection
                .connect(&llm)
                .expect("selection should connect")
                .name(),
            selection.model()
        );
        assert_eq!(
            selection
                .model_info(&llm)
                .expect("selection should have model info")
                .id(),
            selection.model()
        );
    }
}

#[test]
fn missing_account_has_no_fallback() {
    let desk = Desk::fresh();
    let Some(mut llm) = desk.load() else {
        return;
    };
    assert!(matches!(llm.connect(), Err(Error::Unconfigured)));

    llm.set_api_key(backend("openai").into(), "notarealkey");
    llm.select_account(backend("openai").into());
    let selection = Selection::new(backend("codex"), "gpt-5.3-codex", Effort::High);
    assert!(matches!(
        selection.connect(&llm),
        Err(Error::SignedOut(signed_out)) if signed_out == backend("codex").label()
    ));

    llm.set_credential(backend("codex"), Credential::keyed("notarealkey"));
    let codex = account(&llm, "codex");
    assert!(codex.credential().is_none());
    assert!(!codex.is_ready());
}

#[test]
fn removed_backends_are_dropped_from_the_settings() {
    let desk = Desk::fresh();
    if desk.load().is_none() {
        return;
    }
    desk.write_settings(&listing(&[
        ("retired", &[("old-model", false)]),
        ("claude-code", &[("claude-opus-5-5", false)]),
    ]));

    let mut llm = desk.load().expect("the keyring kept running");
    assert_eq!(
        ids(account(&llm, "claude-code").models()),
        ["claude-opus-5-5"]
    );
    llm.set_model(backend("claude-code").into(), "claude-opus-5-5");

    let settings = desk.settings_text();
    assert!(settings.contains("claude-opus-5-5"));
    assert!(!settings.contains("retired"), "{settings}");
}

#[test]
fn listed_models_show_the_latest_until_the_sign_in_changes() {
    let desk = Desk::fresh();
    let Some(mut llm) = desk.load() else {
        return;
    };
    llm.set_credential(backend("antigravity"), antigravity("notarealtoken"));
    assert!(account(&llm, "antigravity").models().is_empty());
    assert!(!account(&llm, "antigravity").has_custom_models());
    desk.write_settings(&listing(&[(
        "antigravity",
        &[("gemini-3-flash", true), ("gemini-4-hydrogen", false)],
    )]));

    let mut read = desk.load().expect("the keyring kept running");
    let listed = account(&read, "antigravity");
    assert_eq!(ids(listed.models()), ["gemini-4-hydrogen"]);
    let superseded = listed
        .find_model("gemini-3-flash")
        .expect("a superseded model stays reachable");
    assert_eq!(superseded.name(), "Named gemini-3-flash");
    let restored = read
        .restore_selection("gemini-3-flash")
        .expect("a superseded model can be restored");
    assert_eq!(restored.account(), backend("antigravity").into());

    read.set_credential(backend("antigravity"), antigravity("anothertoken"));
    assert!(account(&read, "antigravity").models().is_empty());
    assert!(!desk.settings_text().contains("Named gemini-4-hydrogen"));
}
