use rikka_types::Choice;
use serde_json::{Map, Value};

use crate::display::Display;
use crate::error::{Error, Limit, Result};
use crate::unit::Unit;

pub(crate) const MAX_UNITS: usize = 200;
pub(crate) const MAX_SAMPLE_CHARS: usize = 4_000;
const MAX_LANGUAGE_CHARS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Page(PageReport),
    Watch(Watch),
    Translate(Request),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageReport {
    pub lang: Option<String>,
    pub sample: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watch {
    pub top: bool,
    pub session: Option<u64>,
    pub display: Option<Display>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub session: u64,
    pub units: Vec<Unit>,
}

impl Message {
    pub fn parse(body: &Value) -> Result<Message> {
        let object = body.as_object().ok_or(Error::Malformed("message"))?;
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or(Error::Malformed("kind"))?;
        match kind {
            "page" => page(object).map(Message::Page),
            "watch" => watch(object).map(Message::Watch),
            "translate" => request(object).map(Message::Translate),
            other => Err(Error::UnknownKind(other.chars().take(32).collect())),
        }
    }
}

fn page(object: &Map<String, Value>) -> Result<PageReport> {
    let lang = match object.get("lang") {
        None | Some(Value::Null) => None,
        Some(lang) => Some(lang.as_str().ok_or(Error::Malformed("lang"))?.trim()),
    };
    if lang.is_some_and(|lang| lang.chars().count() > MAX_LANGUAGE_CHARS) {
        return Err(Error::TooLarge(Limit::Language));
    }
    let sample = object
        .get("sample")
        .and_then(Value::as_str)
        .ok_or(Error::Malformed("sample"))?;
    if sample.chars().count() > MAX_SAMPLE_CHARS {
        return Err(Error::TooLarge(Limit::Sample));
    }
    Ok(PageReport {
        lang: lang.filter(|lang| !lang.is_empty()).map(str::to_owned),
        sample: sample.to_owned(),
    })
}

fn watch(object: &Map<String, Value>) -> Result<Watch> {
    let top = object
        .get("top")
        .and_then(Value::as_bool)
        .ok_or(Error::Malformed("top"))?;
    let session = match object.get("session") {
        None | Some(Value::Null) => None,
        Some(session) => Some(session.as_u64().ok_or(Error::Malformed("session"))?),
    };
    let display = match object.get("display") {
        None | Some(Value::Null) => None,
        Some(display) => Some(
            display
                .as_str()
                .and_then(Display::from_slug)
                .ok_or(Error::Malformed("display"))?,
        ),
    };
    Ok(Watch {
        top,
        session,
        display,
    })
}

fn request(object: &Map<String, Value>) -> Result<Request> {
    let session = object
        .get("session")
        .and_then(Value::as_u64)
        .ok_or(Error::Malformed("session"))?;
    let units = object
        .get("units")
        .and_then(Value::as_array)
        .ok_or(Error::Malformed("units"))?;
    if units.len() > MAX_UNITS {
        return Err(Error::TooLarge(Limit::Units));
    }
    Ok(Request {
        session,
        units: units.iter().map(Unit::parse).collect::<Result<_>>()?,
    })
}
