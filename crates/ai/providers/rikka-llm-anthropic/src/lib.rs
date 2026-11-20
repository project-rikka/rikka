mod client;

use rikka_llm::{Model, ModelInfo};
use rikka_llm_adapter::{Adapter, Api, Auth, Listing, NoTokens, Result, Service, SignInMethod};

pub use client::{Anthropic, AnthropicApi};

#[derive(Debug)]
pub struct AnthropicAdapter;

impl Adapter for AnthropicAdapter {
    type Tokens = NoTokens;

    const SERVICE: Service<NoTokens> = Service {
        id: "anthropic",
        label: "Anthropic",
        sign_in: SignInMethod::Key {
            variable: AnthropicApi::VARIABLE,
        },
    };

    fn connect(&self, auth: &Auth<'_, NoTokens>, model: &ModelInfo) -> Result<Model> {
        Ok(Anthropic::new(auth.key()?)
            .model(model.id())
            .with(|api| api.listed(model))
            .build()?)
    }

    fn listing(&self, auth: &Auth<'_, NoTokens>) -> Option<Result<Listing>> {
        Some(auth.key().and_then(|key| Ok(client::listing(&key)?)))
    }
}
