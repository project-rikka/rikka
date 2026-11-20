use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Weak};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};

use crate::client::link::{Arrival, Arrivals, Link, LinkError};
use crate::protocol::{
    ErrorObject, InitializeParams, InitializeResult, Listable, Message, Page, RequestId, method,
};

const ABANDONED: &str = "the client stopped waiting";

#[derive(Clone, Debug)]
pub(crate) struct Connection {
    inner: Arc<Inner>,
}

#[derive(Clone, Debug)]
pub(crate) struct WeakConnection {
    inner: Weak<Inner>,
}

#[derive(Debug)]
pub(crate) struct Notification {
    pub(crate) method: String,
    pub(crate) params: Option<Value>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Failure {
    #[error("{}", .0.message)]
    Refused(ErrorObject),

    #[error(transparent)]
    Link(#[from] LinkError),

    #[error("the connection closed before the answer")]
    Closed,

    #[error("{0}")]
    Lost(String),

    #[error("the answer is not the one asked for")]
    Unexpected(#[source] serde_json::Error),
}

type Answering = oneshot::Sender<Result<Value, Failure>>;

#[derive(Debug)]
struct Inner {
    link: Link,
    waiting: Mutex<HashMap<RequestId, Answering>>,
    closed: AtomicBool,
    next_id: AtomicI64,
    runtime: Handle,
}

impl Connection {
    pub(crate) fn open(
        link: Link,
        arrivals: Arrivals,
    ) -> (Connection, mpsc::UnboundedReceiver<Notification>) {
        let inner = Arc::new(Inner {
            link,
            waiting: Mutex::default(),
            closed: AtomicBool::new(false),
            next_id: AtomicI64::new(1),
            runtime: Handle::current(),
        });
        let (noting, notes) = mpsc::unbounded_channel();
        tokio::spawn(dispatch(Arc::downgrade(&inner), arrivals, noting));
        (Connection { inner }, notes)
    }

    pub(crate) fn downgrade(&self) -> WeakConnection {
        WeakConnection {
            inner: Arc::downgrade(&self.inner),
        }
    }

    pub(crate) async fn initialize(&self) -> Result<InitializeResult, Failure> {
        let result: InitializeResult = self
            .request(method::INITIALIZE, InitializeParams::new())
            .await?;
        if !result.speaks_known_revision() {
            tracing::warn!(
                "an MCP server speaks the unknown protocol revision {}",
                result.protocol_version
            );
        }

        self.inner.link.negotiated(&result.protocol_version);
        self.inner
            .link
            .send(&Message::notification(method::INITIALIZED, None))
            .await?;
        self.inner.link.listen();
        Ok(result)
    }

    pub(crate) async fn request<R: DeserializeOwned>(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<R, Failure> {
        let params = match serde_json::to_value(params).map_err(LinkError::Encode)? {
            Value::Null => None,
            params => Some(params),
        };
        let id = RequestId::Number(self.inner.next_id.fetch_add(1, Ordering::Relaxed));
        let (answering, answered) = oneshot::channel();
        self.inner.waiting.lock().insert(id.clone(), answering);
        let mut asked = Asked {
            inner: &self.inner,
            id: id.clone(),
            cancels: method != method::INITIALIZE,
        };
        if self.inner.closed.load(Ordering::Acquire) {
            asked.cancels = false;
            return Err(Failure::Closed);
        }

        let request = Message::request(id, method, params);
        if let Err(error) = self.inner.link.send(&request).await {
            asked.cancels = false;
            return Err(error.into());
        }
        let outcome = answered.await;
        asked.cancels = false;

        let result = outcome.map_err(|_| Failure::Closed)??;
        serde_json::from_value(result).map_err(Failure::Unexpected)
    }

    pub(crate) async fn page<T: Listable>(&self, cursor: Option<&str>) -> Result<Page<T>, Failure> {
        let params = cursor.map(|cursor| json!({ "cursor": cursor }));
        let result: Value = self.request(T::METHOD, params).await?;
        Page::from_result(result).map_err(Failure::Unexpected)
    }

    pub(crate) async fn list_all<T: Listable>(&self) -> Result<Vec<T>, Failure> {
        let mut items = Vec::new();
        let mut cursor = None;
        loop {
            let page = self.page::<T>(cursor.as_deref()).await?;
            items.extend(page.items);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(items);
            }
        }
    }
}

impl WeakConnection {
    pub(crate) fn upgrade(&self) -> Option<Connection> {
        self.inner.upgrade().map(|inner| Connection { inner })
    }
}

struct Asked<'a> {
    inner: &'a Arc<Inner>,
    id: RequestId,
    cancels: bool,
}

impl Drop for Asked<'_> {
    fn drop(&mut self) {
        self.inner.waiting.lock().remove(&self.id);
        if !self.cancels {
            return;
        }

        let inner = Arc::clone(self.inner);
        let cancelled = Message::notification(
            method::CANCELLED,
            Some(json!({ "requestId": self.id, "reason": ABANDONED })),
        );
        self.inner.runtime.spawn(async move {
            if let Err(error) = inner.link.send(&cancelled).await {
                tracing::debug!("failed to cancel a request: {error}");
            }
        });
    }
}

impl Inner {
    fn settle(&self, request: &RequestId, outcome: Result<Value, Failure>) {
        let answering = self.waiting.lock().remove(request);
        if let Some(answering) = answering {
            let _ = answering.send(outcome);
        }
    }

    fn answer(self: &Arc<Self>, id: RequestId, method: &str) {
        let outcome = match method {
            method::PING => Ok(json!({})),
            unknown => Err(ErrorObject::method_not_found(unknown)),
        };
        let inner = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(error) = inner.link.send(&Message::Response { id, outcome }).await {
                tracing::debug!("failed to answer an MCP server: {error}");
            }
        });
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.waiting.lock().clear();
    }
}

async fn dispatch(
    inner: Weak<Inner>,
    mut arrivals: Arrivals,
    noting: mpsc::UnboundedSender<Notification>,
) {
    while let Some(arrival) = arrivals.recv().await {
        let Some(inner) = inner.upgrade() else {
            return;
        };
        match arrival {
            Arrival::Message(Message::Response { id, outcome }) => {
                inner.settle(&id, outcome.map_err(Failure::Refused));
            }
            Arrival::Message(Message::Request { id, method, .. }) => inner.answer(id, &method),
            Arrival::Message(Message::Notification { method, params }) => {
                let _ = noting.send(Notification { method, params });
            }
            Arrival::Lost { request, reason } => inner.settle(&request, Err(Failure::Lost(reason))),
        }
    }
    if let Some(inner) = inner.upgrade() {
        inner.close();
    }
}
