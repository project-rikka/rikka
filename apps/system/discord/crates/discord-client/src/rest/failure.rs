use serde::Deserialize;
use serde_json::Value;

use crate::error::Error;
use discord_translate as translate;
use discord_wire::{MFA_REQUIRED, MfaRequired};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Body {
    message: Option<String>,
    code: Option<u64>,
    errors: Option<Value>,
    captcha_key: Option<Value>,
}

pub(crate) fn failure(status: u16, body: &[u8]) -> Error {
    let parsed: Body = serde_json::from_slice(body).unwrap_or_default();
    if parsed.captcha_key.is_some() {
        return Error::Captcha;
    }
    if parsed.code == Some(MFA_REQUIRED)
        && let Ok(required) = serde_json::from_slice::<MfaRequired>(body)
        && let Some(ticket) = required.mfa
    {
        return Error::Mfa(translate::mfa_challenge(&ticket));
    }
    let mut message = parsed
        .message
        .clone()
        .unwrap_or_else(|| format!("HTTP {status}"));
    if let Some(detail) = parsed.errors.as_ref().and_then(first_detail) {
        message = format!("{message}: {detail}");
    }
    match status {
        401 => Error::SignedOut,
        403 => Error::Forbidden(message),
        404 => Error::Missing(message),
        413 => Error::TooLarge(message),
        400..=499 => Error::Rejected(match parsed.code {
            Some(code) => format!("{message} ({code})"),
            None => message,
        }),
        _ => Error::Unreachable(message),
    }
}

fn first_detail(errors: &Value) -> Option<String> {
    match errors {
        Value::Object(map) => {
            if let Some(Value::Array(listed)) = map.get("_errors") {
                return listed
                    .iter()
                    .find_map(|error| error.get("message")?.as_str().map(str::to_owned));
            }
            map.values().find_map(first_detail)
        }
        Value::Array(items) => items.iter().find_map(first_detail),
        _ => None,
    }
}
