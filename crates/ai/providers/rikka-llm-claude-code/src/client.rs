use rikka_llm::wire::{self, Messages, listing};
use rikka_llm::{Meter, Model, ModelInfo};
use rikka_llm_adapter::{Auth, Listing, Lookup, Result};
use serde_json::{Value, json};

use crate::allowance;
use crate::auth::ClaudeCodeCredentials;
use crate::betas;

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const MODELS: &str = "https://api.anthropic.com/v1/models";
const USAGE_ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";
const VERSION: &str = "2023-06-01";
const AGENT: &str = "claude-cli/2.1.322 (external, cli)";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const ACCEPTS: &str = "application/json, text/plain, */*";
const CATALOG: Lookup = Lookup::Exact("anthropic");

const IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

pub(super) fn connect(auth: &Auth<'_, ClaudeCodeCredentials>, model: &ModelInfo) -> Result<Model> {
    let headers = wire::headers()
        .set("anthropic-version", VERSION)
        .user_agent(AGENT)
        .set("x-app", "cli")
        .json()
        .build()?;

    let transport = auth.transport(wire::endpoint(ENDPOINT)?, headers)?;
    Ok(Messages::new(transport, model.id())
        .listed(model)
        .betas(betas::declared)
        .shape(install)
        .threaded()
        .build())
}

pub(super) fn listing(auth: &Auth<'_, ClaudeCodeCredentials>) -> Result<Listing> {
    let headers = wire::headers()
        .set("anthropic-version", VERSION)
        .set("anthropic-beta", OAUTH_BETA)
        .user_agent(AGENT)
        .accept(ACCEPTS)
        .build()?;
    let transport = auth.transport(wire::endpoint(MODELS)?, headers)?;
    Ok(Listing::new(CATALOG, async move {
        listing::anthropic::models(&transport).await
    }))
}

pub(super) fn meter(auth: &Auth<'_, ClaudeCodeCredentials>) -> Result<Meter> {
    let headers = wire::headers()
        .set("anthropic-beta", OAUTH_BETA)
        .user_agent(AGENT)
        .json()
        .accept(ACCEPTS)
        .build()?;

    let transport = auth.transport(wire::endpoint(USAGE_ENDPOINT)?, headers)?;
    Ok(wire::meter(transport, allowance::read))
}

fn install(body: &mut Value) {
    let identity = json!({ "type": "text", "text": IDENTITY });

    match body.get_mut("system").and_then(Value::as_array_mut) {
        Some(blocks) => blocks.insert(0, identity),
        None => body["system"] = json!([identity]),
    }
}
