use rikka_llm::wire::{self, Responses, ResponsesDialect, listing, random};
use rikka_llm::{Meter, Model, Request};
use rikka_llm_adapter::{Auth, Listing, Lookup, Result};
use rikka_net::HeaderMap;

use crate::allowance;
use crate::auth::CodexCredentials;

macro_rules! release {
    () => {
        "0.169.0"
    };
}

const ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";
const MODELS: &str = concat!(
    "https://chatgpt.com/backend-api/codex/models?client_version=",
    release!()
);
const USAGE_ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";
const AGENT: &str = concat!(
    "codex-tui/",
    release!(),
    " (Mac OS 26.5.0; arm64) iTerm.app/3.6.10 (codex-tui; ",
    release!(),
    ")"
);
const ORIGINATOR: &str = "codex-tui";
const CATALOG: Lookup = Lookup::Exact("openai");

pub(super) fn connect(auth: &Auth<'_, CodexCredentials>, model: &str) -> Result<Model> {
    let headers = wire::headers()
        .user_agent(AGENT)
        .set("originator", ORIGINATOR)
        .keep_alive()
        .json()
        .build()?;

    let transport = auth.transport(wire::endpoint(ENDPOINT)?, headers)?;
    let session = random::uuid();
    Ok(Responses::new(transport, model, ResponsesDialect::Codex)
        .headers(move |request| session_of(request, &session))
        .build())
}

pub(super) fn listing(auth: &Auth<'_, CodexCredentials>) -> Result<Listing> {
    let headers = wire::headers()
        .user_agent(AGENT)
        .set("originator", ORIGINATOR)
        .build()?;
    let transport = auth.transport(wire::endpoint(MODELS)?, headers)?;
    Ok(Listing::new(CATALOG, async move {
        listing::codex::models(&transport).await
    }))
}

pub(super) fn meter(auth: &Auth<'_, CodexCredentials>) -> Result<Meter> {
    let headers = wire::headers().user_agent(AGENT).build()?;
    let transport = auth.transport(wire::endpoint(USAGE_ENDPOINT)?, headers)?;
    Ok(wire::meter(transport, allowance::read))
}

fn session_of(request: &Request, fallback: &str) -> rikka_llm::Result<HeaderMap> {
    let session = request.cache_key().unwrap_or(fallback);
    wire::headers()
        .text("session-id", session, "the session id")
        .build()
}
