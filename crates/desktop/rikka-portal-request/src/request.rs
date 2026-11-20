use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use zbus::blocking::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::error::{Error, Result};

const SUCCESS: u32 = 0;
const CANCELLED: u32 = 1;

static ASKED: AtomicU64 = AtomicU64::new(0);

pub type Results = HashMap<String, OwnedValue>;

#[derive(Debug)]
pub enum Answer {
    Granted(Results),
    Cancelled,
    Ended,
}

#[zbus::proxy(
    interface = "org.freedesktop.portal.Request",
    default_service = "org.freedesktop.portal.Desktop"
)]
trait Request {
    #[zbus(signal)]
    fn response(&self, response: u32, results: Results) -> zbus::Result<()>;
}

#[must_use]
pub fn token() -> String {
    format!(
        "rikka_{}_{}",
        std::process::id(),
        ASKED.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn ask(
    connection: &Connection,
    call: impl FnOnce(&str) -> zbus::Result<OwnedObjectPath>,
) -> Result<Answer> {
    let token = token();
    let handle = handle(connection, &token)?;
    let mut answers = responses(connection, handle.clone())?;
    let answering = call(&token)?;
    let answer = match answering.as_str() == handle {
        true => answers.next(),
        false => {
            tracing::debug!(%answering, %handle, "portal returned a different request handle");
            responses(connection, answering.as_str().to_owned())?.next()
        }
    };
    let answer = answer.ok_or(Error::Silent)?;
    let body = answer
        .args()
        .map_err(|error| Error::Malformed(error.to_string()))?;
    Ok(match body.response {
        SUCCESS => Answer::Granted(body.results),
        CANCELLED => Answer::Cancelled,
        _ => Answer::Ended,
    })
}

fn responses(connection: &Connection, path: String) -> Result<ResponseIterator> {
    Ok(RequestProxyBlocking::builder(connection)
        .path(path)?
        .build()?
        .receive_response()?)
}

fn handle(connection: &Connection, token: &str) -> Result<String> {
    let unique = connection
        .unique_name()
        .ok_or_else(|| Error::Unreachable("connection has no unique bus name".into()))?;
    let sender = unique.trim_start_matches(':').replace('.', "_");
    Ok(format!(
        "/org/freedesktop/portal/desktop/request/{sender}/{token}"
    ))
}
