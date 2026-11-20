mod decode;
mod dialect;
mod encode;
mod stream;

use std::sync::Arc;

use rikka_net::HeaderMap;

use crate::assembler;
use crate::error::Result;
use crate::model::Model;
use crate::provider::{Call, Provider};
use crate::request::Request;
use crate::response::Response;
use crate::stream::Stream;
use crate::transport::Transport;
use crate::vendor::Vendor;

pub use dialect::ResponsesDialect;

type Headers = Arc<dyn Fn(&Request) -> Result<HeaderMap> + Send + Sync>;

#[must_use]
pub struct Responses {
    transport: Transport,
    model: String,
    dialect: ResponsesDialect,
    headers: Option<Headers>,
}

impl Responses {
    pub fn new(
        transport: Transport,
        model: impl Into<String>,
        dialect: ResponsesDialect,
    ) -> Responses {
        Responses {
            transport,
            model: model.into(),
            dialect,
            headers: None,
        }
    }

    pub fn headers(
        mut self,
        headers: impl Fn(&Request) -> Result<HeaderMap> + Send + Sync + 'static,
    ) -> Self {
        self.headers = Some(Arc::new(headers));
        self
    }

    pub fn build(self) -> Model {
        Model::new(Arc::new(self))
    }

    fn headers_for(&self, request: &Request) -> Result<HeaderMap> {
        match &self.headers {
            Some(headers) => headers(request),
            None => Ok(HeaderMap::new()),
        }
    }

    async fn post(&self, request: &Request) -> Result<Response> {
        let model = request.model.as_deref().unwrap_or(&self.model);
        let body = encode::body(request, model, false, self.dialect)?;
        let extra = self.headers_for(request)?;
        let answer = self.transport.post(&body, request, &extra).await?;
        decode::response(&answer, self.dialect.vendor())
    }
}

impl Provider for Responses {
    fn vendor(&self) -> Vendor {
        self.dialect.vendor()
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn send<'a>(&'a self, request: &'a Request) -> Call<'a, Response> {
        Box::pin(async move {
            match self.dialect.streams_only() {
                true => self.stream(request).await?.response().await,
                false => self.post(request).await,
            }
        })
    }

    fn stream<'a>(&'a self, request: &'a Request) -> Call<'a, Stream> {
        Box::pin(async move {
            let model = request.model.as_deref().unwrap_or(&self.model);
            let body = encode::body(request, model, true, self.dialect)?;
            let extra = self.headers_for(request)?;
            let frames = self.transport.events(&body, request, &extra).await?;
            Ok(assembler::assemble(
                frames,
                stream::Assembler::new(self.dialect.vendor()),
            ))
        })
    }
}

impl std::fmt::Debug for Responses {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Responses")
            .field("transport", &self.transport)
            .field("model", &self.model)
            .field("dialect", &self.dialect)
            .finish_non_exhaustive()
    }
}
