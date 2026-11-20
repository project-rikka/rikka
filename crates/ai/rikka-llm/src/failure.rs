use rikka_net::{StatusCode, Url};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::error::Error;

const ERROR_OBJECT: &str = "error";
const ERROR_INFO: &str = "type.googleapis.com/google.rpc.ErrorInfo";
const VALIDATION_REQUIRED: &str = "VALIDATION_REQUIRED";
const VERIFIER: &str = "accounts.google.com";

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Envelope {
    Vendor { error: Reported },
    Codex { detail: String },
    Flat(Flat),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Reported {
    Body(Body),
    Said(String),
}

#[derive(Debug, Deserialize)]
struct Flat {
    message: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "lenient")]
    code: String,
}

impl From<Envelope> for Body {
    fn from(envelope: Envelope) -> Body {
        match envelope {
            Envelope::Vendor {
                error: Reported::Body(error),
            } => error,
            Envelope::Vendor {
                error: Reported::Said(message),
            }
            | Envelope::Codex { detail: message } => Body {
                message,
                ..Body::default()
            },
            Envelope::Flat(Flat {
                message,
                kind,
                code,
            }) => Body {
                message,
                kind,
                code,
                details: Value::Null,
            },
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct Body {
    #[serde(default)]
    message: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "lenient")]
    code: String,
    #[serde(default)]
    details: Value,
}

impl Body {
    fn error_code(&self) -> String {
        self.details
            .get("error_code")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }

    fn verification(&self) -> Option<String> {
        let url = self
            .details
            .as_array()?
            .iter()
            .filter(|detail| detail.get("@type").and_then(Value::as_str) == Some(ERROR_INFO))
            .filter(|detail| {
                detail.get("reason").and_then(Value::as_str) == Some(VALIDATION_REQUIRED)
            })
            .find_map(|detail| detail.pointer("/metadata/validation_url")?.as_str())?;
        Url::parse(url)
            .ok()
            .filter(|url| url.scheme() == "https" && url.host_str() == Some(VERIFIER))
            .map(String::from)
    }
}

#[derive(Debug)]
pub(crate) enum Failure {
    Local(Error),
    Http(rikka_net::Error),
}

#[derive(Debug)]
pub(crate) struct Refusal {
    pub(crate) status: StatusCode,
    pub(crate) code: String,
    pub(crate) message: String,
}

impl Failure {
    pub(crate) fn refusal(&self) -> Option<Refusal> {
        let Failure::Http(error) = self else {
            return None;
        };
        let status = error.status()?;
        let body = read(error);
        Some(Refusal {
            status,
            code: body.error_code(),
            message: body.message,
        })
    }
}

impl From<Error> for Failure {
    fn from(error: Error) -> Failure {
        Failure::Local(error)
    }
}

impl From<rikka_net::Error> for Failure {
    fn from(error: rikka_net::Error) -> Failure {
        Failure::Http(error)
    }
}

impl From<Failure> for Error {
    fn from(failure: Failure) -> Error {
        match failure {
            Failure::Local(error) => error,
            Failure::Http(error) => translate(error),
        }
    }
}

fn lenient<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(code) => code,
        Value::Number(code) => code.to_string(),
        _ => String::new(),
    })
}

pub(crate) fn streamed(chunk: &Value) -> Option<Error> {
    let carried = chunk.get("error").is_some_and(|error| !error.is_null())
        || chunk.get("object").and_then(Value::as_str) == Some(ERROR_OBJECT);
    if !carried {
        return None;
    }
    let body = Envelope::deserialize(chunk)
        .map(Body::from)
        .unwrap_or_default();
    Some(match is_overflow(&body) {
        true => Error::ContextOverflow,
        false => Error::Rejected(describe(&body)),
    })
}

pub fn translate(error: rikka_net::Error) -> Error {
    if error.is_cancelled() {
        return Error::Cancelled;
    }
    if error.is_timeout() {
        return Error::Timeout;
    }
    if error.is_connect() {
        return Error::Connect(Box::new(error));
    }

    let Some(status) = error.status() else {
        return Error::Transport(Box::new(error));
    };

    let retry_after = error.retry_after();
    let body = read(&error);
    from_status(status, retry_after, &body)
}

impl From<rikka_oauth::Error> for Error {
    fn from(error: rikka_oauth::Error) -> Error {
        match error {
            rikka_oauth::Error::Http(error) => translate(error),
            rikka_oauth::Error::TimedOut => Error::Timeout,
            rikka_oauth::Error::ServerResponse(refusal) => match refusal.status() {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Error::Unauthorized,
                StatusCode::TOO_MANY_REQUESTS => Error::RateLimited { retry_after: None },
                _ if refusal.is_expired() => Error::Expired,
                _ => Error::Login(refusal.to_string()),
            },
            rikka_oauth::Error::InvalidHeader(_) => Error::Rejected(error.to_string()),
            rikka_oauth::Error::InvalidUrl(address) => Error::InvalidEndpoint(address),
            error => Error::Login(error.to_string()),
        }
    }
}

fn read(error: &rikka_net::Error) -> Body {
    let rikka_net::Error::Status(status) = error else {
        return Body::default();
    };

    status
        .json::<Envelope>()
        .map(Body::from)
        .unwrap_or_else(|| Body {
            message: status.body().to_owned(),
            ..Body::default()
        })
}

fn is_overflow(body: &Body) -> bool {
    OVERFLOW_KINDS.contains(&body.code.as_str())
        || OVERFLOW_KINDS.contains(&body.kind.as_str())
        || is_too_long(body)
}

const OVERFLOW_KINDS: [&str; 2] = ["context_length_exceeded", "exceed_context_size_error"];

fn from_status(status: StatusCode, retry_after: Option<std::time::Duration>, body: &Body) -> Error {
    if OVERFLOW_KINDS.contains(&body.code.as_str()) || OVERFLOW_KINDS.contains(&body.kind.as_str())
    {
        return Error::ContextOverflow;
    }
    if body.code == "insufficient_quota" || body.code == "billing_hard_limit_reached" {
        return Error::QuotaExceeded;
    }
    if body.code == "model_not_found" {
        return Error::UnknownModel(describe(body));
    }

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => match body.verification() {
            Some(url) => Error::Unverified { url },
            None => Error::Unauthorized,
        },
        StatusCode::NOT_FOUND => Error::UnknownModel(describe(body)),
        StatusCode::PAYMENT_REQUIRED => Error::QuotaExceeded,
        StatusCode::TOO_MANY_REQUESTS => Error::RateLimited { retry_after },
        StatusCode::PAYLOAD_TOO_LARGE => Error::ContextOverflow,
        _ if status.is_server_error() => Error::Overloaded,
        StatusCode::BAD_REQUEST if is_too_long(body) => Error::ContextOverflow,
        _ => Error::Rejected(describe(body)),
    }
}

fn is_too_long(body: &Body) -> bool {
    let message = body.message.to_ascii_lowercase();
    message.contains("prompt is too long")
        || message.contains("too many total text bytes")
        || message.contains("exceed context limit")
        || message.contains("context length is only")
        || message.contains("context length of only")
        || message.contains("maximum context length")
        || message.contains("exceeds the available context size")
        || message.contains("context window")
}

fn describe(body: &Body) -> String {
    match (body.message.is_empty(), body.kind.is_empty()) {
        (false, _) => body.message.clone(),
        (true, false) => body.kind.clone(),
        (true, true) => "no reason given".to_owned(),
    }
}
