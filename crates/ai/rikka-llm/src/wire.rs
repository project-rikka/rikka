use serde_json::Value;

use crate::allowance::{Meter, Scope, Span, Window};

pub use crate::builder::{ResolvedSettings, Settings, endpoint, endpoint_under, key};
pub use crate::cloud_code::CloudCode;
pub use crate::completions::{Completions, CompletionsDialect};
pub use crate::failure::translate;
pub use crate::headers::{Headers, headers};
pub use crate::listing::ListedModel;
pub use crate::messages::Messages;
pub use crate::provider::Call;
pub use crate::responses::{Responses, ResponsesDialect};
pub use crate::transport::{Authorize, Transport, client, count_of, text_of};

pub mod random {
    pub use crate::random::{bytes, hex, uuid};
}

pub mod listing {
    pub use crate::listing::{anthropic, cloud_code, codex, openai};
}

pub type Reader = fn(&Value) -> Vec<Window>;

#[must_use]
pub fn meter(transport: Transport, read: Reader) -> Meter {
    Meter::new(transport, read)
}

#[must_use]
pub fn posted_meter(transport: Transport, body: Value, read: Reader) -> Meter {
    Meter::posting(transport, body, read)
}

#[must_use]
pub fn window(scope: Scope, span: Span, used_percent: f64, resets_at: Option<u64>) -> Window {
    Window::new(scope, span, used_percent, resets_at)
}

#[must_use]
pub fn span_of(seconds: u64) -> Span {
    Span::from_seconds(seconds)
}
