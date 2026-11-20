use std::time::Duration;

use futures_util::future::join_all;
use rikka_llm::wire;
use rikka_llm::{Error, Result};
use rikka_net::{Client, HeaderMap, StatusCode, Url};
use serde_json::{Value, json};

use crate::endpoint::VERSIONED;
use crate::offered::{Described, Offered};

const VERSION: &str = "/api/version";
const SHOW: &str = "/api/show";

const TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

const ARCHITECTURE: &str = "general.architecture";
const CONTEXT_LENGTH: &str = "context_length";
const NUM_CTX: &str = "num_ctx";

const TOOLS: &str = "tools";
const VISION: &str = "vision";
const THINKING: &str = "thinking";

pub(crate) struct Ollama {
    client: Client,
    root: String,
    headers: HeaderMap,
}

impl Ollama {
    pub(crate) async fn recognize(base: &str, headers: HeaderMap) -> Result<Option<Ollama>> {
        let Some(root) = base.strip_suffix(VERSIONED) else {
            return Ok(None);
        };
        let ollama = Ollama {
            client: wire::client(TIMEOUT, CONNECT_TIMEOUT)?,
            root: root.to_owned(),
            headers,
        };
        let Ok(answered) = ollama
            .client
            .get(ollama.under(VERSION)?)
            .headers(ollama.headers.clone())
            .send()
            .await
        else {
            return Ok(None);
        };
        if answered.status() != StatusCode::OK {
            return Ok(None);
        }
        let reported = answered.json::<Value>().await.ok();
        let versioned = reported
            .as_ref()
            .and_then(|body| body.get("version"))
            .is_some_and(Value::is_string);
        Ok(versioned.then_some(ollama))
    }

    pub(crate) async fn described(&self, models: Vec<Offered>) -> Vec<Offered> {
        join_all(models.into_iter().map(|offered| async move {
            match self.show(offered.id()).await {
                Ok(described) => offered.described(described),
                Err(error) => {
                    tracing::warn!(model = offered.id(), %error, "failed to read an Ollama model");
                    offered
                }
            }
        }))
        .await
    }

    async fn show(&self, model: &str) -> Result<Described> {
        let body: Value = self
            .client
            .post(self.under(SHOW)?)
            .headers(self.headers.clone())
            .json(&json!({ "model": model }))
            .send()
            .await
            .map_err(wire::translate)?
            .error_for_status()
            .await
            .map_err(wire::translate)?
            .json()
            .await
            .map_err(wire::translate)?;
        Ok(read(&body))
    }

    fn under(&self, path: &str) -> Result<Url> {
        Url::parse(&format!("{}{path}", self.root))
            .map_err(|_| Error::InvalidEndpoint(self.root.clone()))
    }
}

fn read(shown: &Value) -> Described {
    let capabilities = shown.get("capabilities").and_then(Value::as_array);
    let capable = |named: &str| {
        capabilities.map(|listed| listed.iter().any(|held| held.as_str() == Some(named)))
    };
    Described {
        context: configured_context(shown).or_else(|| trained_context(shown)),
        vision: capable(VISION),
        tools: capable(TOOLS),
        reasoning: capable(THINKING),
    }
}

fn configured_context(shown: &Value) -> Option<u64> {
    shown
        .get("parameters")
        .and_then(Value::as_str)?
        .lines()
        .filter_map(|line| line.trim().split_once(char::is_whitespace))
        .find(|(name, _)| *name == NUM_CTX)
        .and_then(|(_, value)| value.trim().parse().ok())
        .filter(|tokens| *tokens > 0)
}

fn trained_context(shown: &Value) -> Option<u64> {
    let info = shown.get("model_info")?;
    let architecture = info.get(ARCHITECTURE).and_then(Value::as_str)?;
    info.get(format!("{architecture}.{CONTEXT_LENGTH}"))
        .and_then(Value::as_u64)
        .filter(|tokens| *tokens > 0)
}
