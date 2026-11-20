mod client;

use rikka_llm::wire::{self, Transport, listing};
use rikka_llm::{Model, ModelInfo};
use rikka_llm_adapter::{
    Adapter, Api, Auth, Listing, Lookup, NoTokens, Result, Service, SignInMethod,
};

pub use client::{OpenAi, OpenAiApi};

const MODELS: &str = "https://api.openai.com/v1/models";
const CATALOG: Lookup = Lookup::Exact("openai");

#[derive(Debug)]
pub struct OpenAiAdapter;

impl Adapter for OpenAiAdapter {
    type Tokens = NoTokens;

    const SERVICE: Service<NoTokens> = Service {
        id: "openai",
        label: "OpenAI",
        sign_in: SignInMethod::Key {
            variable: OpenAiApi::VARIABLE,
        },
    };

    fn connect(&self, auth: &Auth<'_, NoTokens>, model: &ModelInfo) -> Result<Model> {
        Ok(OpenAi::new(auth.key()?).model(model.id()).build()?)
    }

    fn listing(&self, auth: &Auth<'_, NoTokens>) -> Option<Result<Listing>> {
        Some(auth.key().and_then(|key| Ok(listed(&key)?)))
    }
}

fn listed(key: &str) -> rikka_llm::Result<Listing> {
    let headers = wire::headers().bearer(key).build()?;
    let transport = Transport::new(wire::endpoint(MODELS)?, headers)?;
    Ok(Listing::new(CATALOG, async move {
        listing::openai::models(&transport).await
    }))
}
