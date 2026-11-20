mod decode;
mod dialect;
mod encode;
mod stream;
mod thinking;

use std::sync::Arc;

use rikka_net::HeaderMap;
use serde_json::Value;

use crate::assembler;
use crate::completions::encode::Ceiling;
use crate::model::Model;
use crate::provider::{Call, Provider};
use crate::request::Request;
use crate::response::Response;
use crate::stream::Stream;
use crate::transport::Transport;
use crate::vendor::Vendor;

pub use dialect::CompletionsDialect;

#[must_use]
#[derive(Debug)]
pub struct Completions {
    transport: Transport,
    model: String,
    ceiling: Ceiling,
    dialect: CompletionsDialect,
}

impl Completions {
    pub fn new(
        transport: Transport,
        model: impl Into<String>,
        dialect: CompletionsDialect,
    ) -> Completions {
        Completions {
            transport,
            model: model.into(),
            ceiling: Ceiling::Completion,
            dialect,
        }
    }

    pub fn legacy_max_tokens(mut self) -> Self {
        self.ceiling = Ceiling::Legacy;
        self
    }

    pub fn build(self) -> Model {
        Model::new(Arc::new(self))
    }

    fn body(&self, request: &Request, stream: bool) -> Value {
        let model = request.model.as_deref().unwrap_or(&self.model);
        encode::body(request, model, self.ceiling, self.dialect, stream)
    }
}

impl Provider for Completions {
    fn vendor(&self) -> Vendor {
        self.dialect.vendor()
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn send<'a>(&'a self, request: &'a Request) -> Call<'a, Response> {
        Box::pin(async move {
            let body = self.body(request, false);
            decode::response(
                &self
                    .transport
                    .post(&body, request, &HeaderMap::new())
                    .await?,
                self.dialect,
            )
        })
    }

    fn stream<'a>(&'a self, request: &'a Request) -> Call<'a, Stream> {
        Box::pin(async move {
            let body = self.body(request, true);
            let frames = self
                .transport
                .events(&body, request, &HeaderMap::new())
                .await?;
            Ok(assembler::assemble(
                frames,
                stream::Assembler::new(self.dialect),
            ))
        })
    }
}
