use std::sync::Arc;

use crate::error::Result;
use crate::provider::Provider;
use crate::request::Request;
use crate::request_builder::RequestBuilder;
use crate::response::Response;
use crate::stream::Stream;
use crate::vendor::Vendor;

pub(crate) const DEFAULT_MAX_TOKENS: u32 = 8192;

#[derive(Debug, Clone)]
pub struct Model {
    provider: Arc<dyn Provider>,
}

impl Model {
    pub(crate) fn new(provider: Arc<dyn Provider>) -> Model {
        Model { provider }
    }

    #[must_use]
    pub fn vendor(&self) -> Vendor {
        self.provider.vendor()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.provider.model()
    }

    pub fn chat(&self) -> RequestBuilder {
        RequestBuilder::new(self.clone())
    }

    pub async fn ask(&self, prompt: impl Into<String>) -> Result<String> {
        Ok(self.chat().user(prompt.into()).send().await?.text())
    }

    pub async fn send(&self, request: &Request) -> Result<Response> {
        self.provider.send(request).await
    }

    pub async fn stream(&self, request: &Request) -> Result<Stream> {
        self.provider.stream(request).await
    }
}
