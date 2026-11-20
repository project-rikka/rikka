use rikka_llm::wire::{self, Messages, Transport, listing};
use rikka_llm::{Model, ModelInfo, Result};
use rikka_llm_adapter::{Api, Keyed, KeyedConnection, Listing, Lookup};

const DEFAULT_VERSION: &str = "2026-10-21";
const MODELS: &str = "https://api.anthropic.com/v1/models";
const CATALOG: Lookup = Lookup::Exact("anthropic");

pub type Anthropic = Keyed<AnthropicApi>;

#[must_use]
#[derive(Debug)]
pub struct AnthropicApi {
    version: String,
    betas: Vec<String>,
    max_tokens: Option<u32>,
    listed: Option<ModelInfo>,
}

impl Default for AnthropicApi {
    fn default() -> AnthropicApi {
        AnthropicApi {
            version: DEFAULT_VERSION.to_owned(),
            betas: Vec::new(),
            max_tokens: None,
            listed: None,
        }
    }
}

impl AnthropicApi {
    pub fn version(mut self, version: impl Into<String>) -> AnthropicApi {
        self.version = version.into();
        self
    }

    pub fn beta(mut self, flag: impl Into<String>) -> AnthropicApi {
        self.betas.push(flag.into());
        self
    }

    pub fn max_tokens(mut self, tokens: u32) -> AnthropicApi {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn listed(mut self, model: &ModelInfo) -> AnthropicApi {
        self.listed = Some(model.clone());
        self
    }
}

impl Api for AnthropicApi {
    const VARIABLE: &'static str = "ANTHROPIC_API_KEY";

    const ENDPOINT: &'static str = "https://api.anthropic.com/v1/messages";

    fn connect(self, connection: KeyedConnection) -> Result<Model> {
        let KeyedConnection {
            model,
            key,
            endpoint,
            headers,
        } = connection;
        let headers = headers
            .bearer(&key)
            .text("anthropic-version", &self.version, "the API version")
            .json()
            .build()?;

        let betas = self.betas.join(",");
        let messages = Messages::new(Transport::new(endpoint, headers)?, model)
            .max_tokens(self.max_tokens)
            .betas(move |_, _| betas.clone())
            .threaded();
        Ok(match &self.listed {
            Some(listed) => messages.listed(listed),
            None => messages,
        }
        .build())
    }
}

pub(crate) fn listing(key: &str) -> Result<Listing> {
    let headers = wire::headers()
        .bearer(key)
        .text("anthropic-version", DEFAULT_VERSION, "the API version")
        .build()?;
    let transport = Transport::new(wire::endpoint(MODELS)?, headers)?;
    Ok(Listing::new(CATALOG, async move {
        listing::anthropic::models(&transport).await
    }))
}
