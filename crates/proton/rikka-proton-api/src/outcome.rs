use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::code::ResponseCode;
use crate::error::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct NoBody {}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "Id: Deserialize<'de>, Body: Deserialize<'de> + Default"))]
pub struct Outcome<Id, Body = NoBody> {
    #[serde(rename = "ID", alias = "LinkID", default = "Option::default")]
    id: Option<Id>,
    #[serde(rename = "Index", default)]
    index: usize,
    #[serde(rename = "Response", default)]
    reply: Reply<Body>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "Body: Deserialize<'de> + Default"))]
struct Reply<Body> {
    #[serde(rename = "Code", default = "succeeded")]
    code: ResponseCode,
    #[serde(rename = "Error", default)]
    error: Option<String>,
    #[serde(rename = "Details", default)]
    details: Option<Value>,
    #[serde(flatten)]
    body: Body,
}

impl<Body: Default> Default for Reply<Body> {
    fn default() -> Reply<Body> {
        Reply {
            code: ResponseCode::Ok,
            error: None,
            details: None,
            body: Body::default(),
        }
    }
}

fn succeeded() -> ResponseCode {
    ResponseCode::Ok
}

impl<Id, Body: Default> Outcome<Id, Body> {
    #[must_use]
    pub fn succeeded(id: Id) -> Outcome<Id, Body> {
        Outcome {
            id: Some(id),
            index: 0,
            reply: Reply::default(),
        }
    }

    #[must_use]
    pub fn refused(id: Id, error: &Error) -> Option<Outcome<Id, Body>> {
        match error {
            Error::Api(refusal) => Some(Outcome {
                id: Some(id),
                index: 0,
                reply: Reply {
                    code: refusal.code,
                    error: Some(refusal.message.clone()),
                    details: refusal.details.clone(),
                    body: Body::default(),
                },
            }),
            _ => None,
        }
    }
}

impl<Id, Body> Outcome<Id, Body> {
    #[must_use]
    pub fn id(&self) -> Option<&Id> {
        self.id.as_ref()
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn code(&self) -> ResponseCode {
        self.reply.code
    }

    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.reply.code.is_ok()
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.reply
            .error
            .as_deref()
            .filter(|error| !error.is_empty())
    }

    #[must_use]
    pub fn body(&self) -> &Body {
        &self.reply.body
    }

    #[must_use]
    pub fn detail<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let detail = self.reply.details.as_ref()?.get(name)?;
        T::deserialize(detail).ok()
    }

    #[must_use]
    pub fn offset_by(mut self, first: usize) -> Outcome<Id, Body> {
        self.index += first;
        self
    }
}
