mod decode;
mod encode;
mod schema;
mod stream;

use std::borrow::Cow;
use std::sync::Arc;

use rikka_net::{HeaderMap, HeaderValue, Url, header};

use crate::assembler;
use crate::effort::Effort;
use crate::model::Model;
use crate::model_info::ModelInfo;
use crate::provider::{Call, Provider};
use crate::request::Request;
use crate::response::Response;
use crate::stream::Stream;
use crate::transport::Transport;
use crate::vendor::Vendor;

#[must_use]
#[derive(Debug)]
pub struct CloudCode {
    transport: Transport,
    streaming: Transport,
    model: String,
    levels: Vec<Effort>,
    project: String,
    session: String,
}

impl CloudCode {
    pub fn new(transport: Transport, model: impl Into<String>) -> CloudCode {
        CloudCode {
            streaming: transport.clone(),
            transport,
            model: model.into(),
            levels: Vec::new(),
            project: String::new(),
            session: String::new(),
        }
    }

    pub fn listed(mut self, model: &ModelInfo) -> Self {
        self.levels = model.efforts().to_vec();
        self
    }

    pub fn streaming_at(mut self, endpoint: Url) -> Self {
        self.streaming = self.transport.with_endpoint(endpoint);
        self
    }

    pub fn project(mut self, project: impl Into<String>) -> Self {
        self.project = project.into();
        self
    }

    pub fn session(mut self, session: impl Into<String>) -> Self {
        self.session = session.into();
        self
    }

    pub fn build(self) -> Model {
        Model::new(Arc::new(self))
    }

    fn model_for<'a>(&'a self, request: &'a Request) -> Cow<'a, str> {
        if let Some(model) = request.model.as_deref() {
            return Cow::Borrowed(model);
        }
        let wanted = request.effort.unwrap_or_default();
        let level = self
            .levels
            .iter()
            .rev()
            .find(|level| **level <= wanted)
            .or(self.levels.first());
        match level {
            Some(level) => Cow::Owned(format!("{}-{}", self.model, level.id())),
            None => Cow::Borrowed(&self.model),
        }
    }
}

impl Provider for CloudCode {
    fn vendor(&self) -> Vendor {
        Vendor::Antigravity
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn send<'a>(&'a self, request: &'a Request) -> Call<'a, Response> {
        Box::pin(async move {
            let model = self.model_for(request);
            let body = encode::body(request, &model, &self.project, &self.session);
            let answer = self.transport.post(&body, request, &accepting()).await?;

            decode::response(&answer)
        })
    }

    fn stream<'a>(&'a self, request: &'a Request) -> Call<'a, Stream> {
        Box::pin(async move {
            let model = self.model_for(request);
            let body = encode::body(request, &model, &self.project, &self.session);
            let events = self
                .streaming
                .events(&body, request, &HeaderMap::new())
                .await?;

            Ok(assembler::assemble(events, stream::Assembler::default()))
        })
    }
}

fn accepting() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT, HeaderValue::from_static("*/*"));
    headers
}
