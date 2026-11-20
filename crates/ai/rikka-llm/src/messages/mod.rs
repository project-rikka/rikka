mod decode;
mod encode;
mod hosted;
mod stream;
mod thread;

pub(crate) use hosted::part as hosted_part;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use rikka_net::HeaderMap;
use serde_json::Value;

use crate::assembler;
use crate::breakpoint::Placement;
use crate::error::Result;
use crate::failure::Failure;
use crate::headers;
use crate::messages::encode::{Encoding, Room};
use crate::messages::thread::{Key, Lapse, Ledger, Recording, Step};
use crate::model::Model;
use crate::model_info::ModelInfo;
use crate::provider::{Call, Provider};
use crate::request::Request;
use crate::response::Response;
use crate::server_tool::ServerTool;
use crate::stream::Stream;
use crate::thinking_off::ThinkingOff;
use crate::transport::{Events, Transport};
use crate::vendor::Vendor;

type Betas = Arc<dyn Fn(&Request, &str) -> String + Send + Sync>;
type Delivery<'a, T> = Pin<Box<dyn Future<Output = Result<T, Failure>> + Send + 'a>>;

#[must_use]
pub struct Messages {
    transport: Transport,
    model: String,
    max_tokens: Option<u32>,
    listed_output: Option<u32>,
    thinking_off: ThinkingOff,
    server_tools: Vec<ServerTool>,
    betas: Option<Betas>,
    shape: fn(&mut Value),
    threaded: bool,
}

impl Messages {
    pub fn new(transport: Transport, model: impl Into<String>) -> Messages {
        Messages {
            transport,
            model: model.into(),
            max_tokens: None,
            listed_output: None,
            thinking_off: ThinkingOff::Disabled,
            server_tools: Vec::new(),
            betas: None,
            shape: |_| {},
            threaded: false,
        }
    }

    pub fn max_tokens(mut self, tokens: Option<u32>) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn listed(mut self, model: &ModelInfo) -> Self {
        self.listed_output = Some(model.output()).filter(|tokens| *tokens > 0);
        self.thinking_off = model.thinking_off();
        self.server_tools = model.server_tools().to_vec();
        self
    }

    pub fn betas(
        mut self,
        betas: impl Fn(&Request, &str) -> String + Send + Sync + 'static,
    ) -> Self {
        self.betas = Some(Arc::new(betas));
        self
    }

    pub fn shape(mut self, shape: fn(&mut Value)) -> Self {
        self.shape = shape;
        self
    }

    pub fn threaded(mut self) -> Self {
        self.threaded = true;
        self
    }

    pub fn build(self) -> Model {
        Model::new(Arc::new(self))
    }

    fn body<D: Deliver>(
        &self,
        request: &Request,
        model: &str,
        placement: Placement,
    ) -> Result<Value> {
        let configured = model == self.model;
        let server_tools = match configured {
            true => request
                .server_tools
                .iter()
                .copied()
                .filter(|tool| self.server_tools.contains(tool))
                .collect(),
            false => Vec::new(),
        };
        let encoding = Encoding {
            room: Room::new(self.max_tokens, self.listed_output.filter(|_| configured)),
            stream: D::STREAM,
            placement,
            thinking_off: match configured {
                true => self.thinking_off,
                false => ThinkingOff::Disabled,
            },
            server_tools: &server_tools,
        };
        let mut body = encode::body(request, model, encoding)?;
        (self.shape)(&mut body);
        Ok(body)
    }

    fn declared(&self, request: &Request, model: &str) -> String {
        self.betas
            .as_ref()
            .map(|betas| betas(request, model))
            .unwrap_or_default()
    }

    async fn deliver<D: Deliver>(
        &self,
        request: &Request,
        body: &Value,
        betas: &str,
    ) -> Result<D::Output, Failure> {
        let headers = headers::beta(betas)?;
        D::deliver(&self.transport, body, request, &headers).await
    }

    async fn exchange<D: Deliver>(
        &self,
        request: &Request,
    ) -> Result<(D::Output, Option<Recording>)> {
        let model = request.model.as_deref().unwrap_or(&self.model);
        let declared = self.declared(request, model);
        let conversation = request
            .cache_key
            .as_deref()
            .filter(|_| self.threaded)
            .map(|conversation| Key::new(self.transport.endpoint(), conversation));
        let Some(key) = conversation.filter(|key| !key.is_unthreaded()) else {
            return self.unthreaded::<D>(request, model, &declared).await;
        };

        let mut body = self.body::<D>(request, model, Placement::Automatic)?;
        let betas = match declared.is_empty() {
            true => thread::BETA.to_owned(),
            false => format!("{declared},{}", thread::BETA),
        };
        let ledger = Ledger::new(&body, &betas);

        let recovered = match key.step(&ledger) {
            Step::Create => false,
            Step::Continue(open) => {
                let continued = thread::continuation(&body, open.from(), open.id());
                let sent = Instant::now();
                match self.deliver::<D>(request, &continued, &betas).await {
                    Ok(output) => {
                        return Ok((output, Some(Recording::new(key, ledger, sent, false))));
                    }
                    Err(failure) => match thread::lapse(&failure) {
                        Some(Lapse::Stale) => true,
                        Some(Lapse::Missing) if !open.is_recovered() => true,
                        Some(Lapse::Missing | Lapse::Unsupported) => {
                            key.forsake();
                            return self.plain::<D>(request, &body, &declared).await;
                        }
                        None => {
                            key.restore(open);
                            return Err(failure.into());
                        }
                    },
                }
            }
        };

        thread::creation(&mut body);
        let sent = Instant::now();
        match self.deliver::<D>(request, &body, &betas).await {
            Ok(output) => Ok((output, Some(Recording::new(key, ledger, sent, recovered)))),
            Err(failure) if thread::lapse(&failure) == Some(Lapse::Unsupported) => {
                key.forsake();
                thread::unthreaded(&mut body);
                self.plain::<D>(request, &body, &declared).await
            }
            Err(failure) => Err(failure.into()),
        }
    }

    async fn unthreaded<D: Deliver>(
        &self,
        request: &Request,
        model: &str,
        declared: &str,
    ) -> Result<(D::Output, Option<Recording>)> {
        let body = self.body::<D>(request, model, Placement::Blocks)?;
        self.plain::<D>(request, &body, declared).await
    }

    async fn plain<D: Deliver>(
        &self,
        request: &Request,
        body: &Value,
        declared: &str,
    ) -> Result<(D::Output, Option<Recording>)> {
        let output = self.deliver::<D>(request, body, declared).await?;
        Ok((output, None))
    }
}

trait Deliver {
    type Output: Send;

    const STREAM: bool;

    fn deliver<'a>(
        transport: &'a Transport,
        body: &'a Value,
        request: &'a Request,
        headers: &'a HeaderMap,
    ) -> Delivery<'a, Self::Output>;
}

struct Answer;

struct Streamed;

impl Deliver for Answer {
    type Output = Value;

    const STREAM: bool = false;

    fn deliver<'a>(
        transport: &'a Transport,
        body: &'a Value,
        request: &'a Request,
        headers: &'a HeaderMap,
    ) -> Delivery<'a, Value> {
        Box::pin(transport.try_post(body, request, headers))
    }
}

impl Deliver for Streamed {
    type Output = Events;

    const STREAM: bool = true;

    fn deliver<'a>(
        transport: &'a Transport,
        body: &'a Value,
        request: &'a Request,
        headers: &'a HeaderMap,
    ) -> Delivery<'a, Events> {
        Box::pin(transport.try_events(body, request, headers))
    }
}

impl Provider for Messages {
    fn vendor(&self) -> Vendor {
        Vendor::Anthropic
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn send<'a>(&'a self, request: &'a Request) -> Call<'a, Response> {
        Box::pin(async move {
            let (body, recording) = self.exchange::<Answer>(request).await?;
            let response = decode::response(&body)?;
            if let Some(recording) = recording {
                recording.record(&response);
            }
            Ok(response)
        })
    }

    fn stream<'a>(&'a self, request: &'a Request) -> Call<'a, Stream> {
        Box::pin(async move {
            let (frames, recording) = self.exchange::<Streamed>(request).await?;
            let stream = assembler::assemble(frames, stream::Assembler::default());
            Ok(match recording {
                Some(recording) => thread::recorded(stream, recording),
                None => stream,
            })
        })
    }
}

impl std::fmt::Debug for Messages {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Messages")
            .field("transport", &self.transport)
            .field("model", &self.model)
            .field("max_tokens", &self.max_tokens)
            .field("threaded", &self.threaded)
            .finish_non_exhaustive()
    }
}
