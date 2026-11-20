use rikka_llm::wire::{Completions, CompletionsDialect, Transport};
use rikka_llm::{Model, Result};
use rikka_llm_adapter::{Api, Keyed, KeyedConnection};
use rikka_net::header;

pub type OpenRouter = Keyed<OpenRouterApi>;

#[must_use]
#[derive(Debug, Default)]
pub struct OpenRouterApi {
    app: Option<App>,
}

#[derive(Debug)]
struct App {
    name: String,
    url: String,
}

impl OpenRouterApi {
    pub fn app(mut self, name: impl Into<String>, url: impl Into<String>) -> OpenRouterApi {
        self.app = Some(App {
            name: name.into(),
            url: url.into(),
        });
        self
    }
}

impl Api for OpenRouterApi {
    const VARIABLE: &'static str = "OPENROUTER_API_KEY";

    const ENDPOINT: &'static str = "https://openrouter.ai/api/v1/chat/completions";

    fn connect(self, connection: KeyedConnection) -> Result<Model> {
        let KeyedConnection {
            model,
            key,
            endpoint,
            headers,
        } = connection;

        let mut headers = headers.bearer(&key).json();
        if let Some(app) = &self.app {
            headers = headers
                .text(header::REFERER, &app.url, "the app's URL")
                .text("x-title", &app.name, "the app's name");
        }

        Ok(Completions::new(
            Transport::new(endpoint, headers.build()?)?,
            model,
            CompletionsDialect::OpenRouter,
        )
        .legacy_max_tokens()
        .build())
    }
}
