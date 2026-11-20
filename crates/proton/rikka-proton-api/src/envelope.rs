use std::time::Duration;

use http::StatusCode;
use serde::Deserialize;

use crate::code::ResponseCode;
use crate::details::{Details, DeviceVerificationDetails, HumanVerificationDetails};
use crate::error::{ApiError, Error};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Envelope {
    #[serde(rename = "Code")]
    code: Option<i32>,
    #[serde(rename = "Error")]
    error: Option<String>,
    #[serde(rename = "ErrorDescription")]
    description: Option<String>,
    #[serde(rename = "Details")]
    details: Option<serde_json::Value>,
}

impl Envelope {
    pub(crate) fn read(body: &[u8]) -> Option<Envelope> {
        serde_json::from_slice::<Envelope>(body)
            .ok()
            .filter(|envelope| envelope.code.is_some())
    }

    pub(crate) fn code(&self) -> Option<ResponseCode> {
        self.code.map(ResponseCode::new)
    }

    fn message(&self) -> String {
        self.error
            .clone()
            .or_else(|| self.description.clone())
            .unwrap_or_else(|| "API error without message".to_owned())
    }

    fn details_as<T: for<'de> Deserialize<'de> + Default>(&self) -> T {
        self.details
            .clone()
            .and_then(|details| serde_json::from_value(details).ok())
            .unwrap_or_default()
    }
}

pub(crate) fn classify(status: StatusCode, retry_after: Option<Duration>, body: &[u8]) -> Error {
    let envelope = Envelope::read(body).unwrap_or_default();

    match envelope.code() {
        Some(ResponseCode::HumanVerificationRequired) => {
            return Error::HumanVerificationRequired(
                envelope.details_as::<HumanVerificationDetails>(),
            );
        }
        Some(ResponseCode::DeviceVerificationRequired) => {
            return Error::DeviceVerificationRequired(
                envelope.details_as::<DeviceVerificationDetails>(),
            );
        }
        Some(code) if code.is_force_update() => return Error::ForceUpdate,
        Some(code) if code.is_missing_scope() && status == StatusCode::FORBIDDEN => {
            let scopes = envelope.details_as::<Details>().missing_scopes;
            if !scopes.is_empty() {
                return Error::MissingScope(scopes);
            }
        }
        _ => {}
    }

    match status {
        StatusCode::TOO_MANY_REQUESTS => Error::TooManyRequests { retry_after },
        StatusCode::UNAUTHORIZED => Error::Unauthorized,
        _ => match envelope.code() {
            Some(code) => Error::Api(Box::new(ApiError {
                status,
                code,
                message: envelope.message(),
                details: envelope.details.clone(),
            })),
            None => Error::Status {
                status,
                body: String::from_utf8_lossy(body).into_owned(),
            },
        },
    }
}
