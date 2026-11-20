use std::fmt;

use rikka_net::StatusCode;
use serde_json::Value;

const MAX_EXCERPT_CHARS: usize = 200;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to listen on {address}: {source}")]
    Listen {
        address: String,
        #[source]
        source: std::io::Error,
    },

    #[error("authorization timed out")]
    TimedOut,

    #[error("access denied")]
    AccessDenied,

    #[error("authorization failed: {0}")]
    Failed(String),

    #[error("{0}")]
    ServerResponse(ErrorResponse),

    #[error("authorization server response is missing {0}")]
    MissingField(&'static str),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("protected resource metadata describes {found}, not {expected}")]
    ResourceMismatch { expected: String, found: String },

    #[error("authorization server is {found}, not {expected}")]
    IssuerMismatch { expected: String, found: String },

    #[error("authorization server did not name itself in its answer")]
    MissingIssuer,

    #[error("{0} contains bytes invalid in a header")]
    InvalidHeader(&'static str),

    #[error(transparent)]
    Http(#[from] rikka_net::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorResponse {
    status: StatusCode,
    code: String,
    description: String,
    body_excerpt: String,
}

impl ErrorResponse {
    pub(crate) fn from_response(status: StatusCode, body: &Value) -> ErrorResponse {
        ErrorResponse {
            status,
            code: text(body, "error"),
            description: text(body, "error_description"),
            body_excerpt: excerpt(body),
        }
    }

    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        matches!(self.code.as_str(), "invalid_grant" | "refresh_token_reused")
    }
}

impl fmt::Display for ErrorResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.code.as_str(), self.description.as_str()) {
            ("", "") if self.body_excerpt.is_empty() => {
                write!(formatter, "authorization server returned {}", self.status)
            }
            ("", "") => write!(
                formatter,
                "authorization server returned {}: {}",
                self.status, self.body_excerpt
            ),
            (code, "") => formatter.write_str(code),
            ("", description) => formatter.write_str(description),
            (code, description) => write!(formatter, "{code}: {description}"),
        }
    }
}

pub(crate) fn texts(body: &Value, field: &str) -> Option<Vec<String>> {
    let listed = body.get(field)?.as_array()?;
    Some(
        listed
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}

pub(crate) fn text(body: &Value, field: &str) -> String {
    body.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn excerpt(body: &Value) -> String {
    let Some(text) = body.as_str().map(str::trim).filter(|text| !text.is_empty()) else {
        return String::new();
    };

    match text.char_indices().nth(MAX_EXCERPT_CHARS) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_owned(),
    }
}
