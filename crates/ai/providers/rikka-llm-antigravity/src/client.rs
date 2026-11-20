use serde_json::json;

use rikka_llm::wire::{self, CloudCode, listing, random};
use rikka_llm::{Error, Meter, Model, ModelInfo};
use rikka_llm_adapter::{Auth, Listing, Lookup, Result};

use crate::allowance;
use crate::auth::AntigravityCredentials;
use crate::hub;

const BASE_URL: &str = "https://daily-cloudcode-pa.googleapis.com";
const GENERATE: &str = "/v1internal:generateContent";
const STREAM: &str = "/v1internal:streamGenerateContent?alt=sse";
const MODELS: &str = "/v1internal:fetchAvailableModels";
const QUOTA_SUMMARY: &str = "/v1internal:retrieveUserQuotaSummary";
const CATALOG: Lookup = Lookup::Prefix(&["google", "anthropic", "openai"]);

pub(super) fn connect(auth: &Auth<'_, AntigravityCredentials>, model: &ModelInfo) -> Result<Model> {
    let project = project(auth)?;
    let headers = wire::headers().json().build()?;

    let generate = wire::endpoint_under(BASE_URL, GENERATE)?;
    let stream = wire::endpoint_under(BASE_URL, STREAM)?;
    let transport = auth.transport(generate, headers)?;
    Ok(CloudCode::new(transport, model.id())
        .listed(model)
        .streaming_at(stream)
        .project(project)
        .session(session())
        .build())
}

pub(super) fn listing(auth: &Auth<'_, AntigravityCredentials>) -> Result<Listing> {
    let project = project(auth)?;
    let headers = wire::headers().json().build()?;
    let transport = auth.transport(wire::endpoint_under(BASE_URL, MODELS)?, headers)?;
    Ok(Listing::new(CATALOG, async move {
        hub::refresh().await;
        listing::cloud_code::models(&transport, &project).await
    }))
}

pub(super) fn meter(auth: &Auth<'_, AntigravityCredentials>) -> Result<Meter> {
    let project = project(auth)?;
    let headers = wire::headers().json().build()?;
    let transport = auth.transport(wire::endpoint_under(BASE_URL, QUOTA_SUMMARY)?, headers)?;
    Ok(wire::posted_meter(
        transport,
        json!({ "project": project }),
        allowance::read,
    ))
}

fn project(auth: &Auth<'_, AntigravityCredentials>) -> Result<String> {
    let project = auth.tokens()?.project_id().to_owned();
    if project.is_empty() {
        return Err(Error::Rejected("the credentials name no project to bill".to_owned()).into());
    }
    Ok(project)
}

fn session() -> String {
    format!("-{}", u64::from_le_bytes(random::bytes::<8>()) >> 4)
}
