use std::collections::HashMap;
use std::os::fd::OwnedFd;

use rikka_portal_request::{Answer, Results};
use zbus::blocking::Connection;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use crate::error::{Error, Result};
use crate::source::{SourceKind, Sources};

const EMBEDDED_CURSOR: u32 = 2;
const HIDDEN_CURSOR: u32 = 1;
const PERSIST_UNTIL_REVOKED: u32 = 2;

#[zbus::proxy(
    interface = "org.freedesktop.portal.ScreenCast",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait ScreenCast {
    fn create_session(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<OwnedObjectPath>;

    fn select_sources(
        &self,
        session_handle: &ObjectPath<'_>,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;

    fn start(
        &self,
        session_handle: &ObjectPath<'_>,
        parent_window: &str,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;

    fn open_pipe_wire_remote(
        &self,
        session_handle: &ObjectPath<'_>,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;

    #[zbus(property)]
    fn available_source_types(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn available_cursor_modes(&self) -> zbus::Result<u32>;
}

#[zbus::proxy(
    interface = "org.freedesktop.portal.Session",
    default_service = "org.freedesktop.portal.Desktop"
)]
trait Session {
    fn close(&self) -> zbus::Result<()>;
}

pub(crate) struct CastRequest {
    pub(crate) sources: Sources,
    pub(crate) cursor: bool,
    pub(crate) persist: bool,
    pub(crate) restore: Option<String>,
}

pub(crate) struct CastSession {
    pub(crate) session: Shared,
    pub(crate) node: u32,
    pub(crate) kind: SourceKind,
    pub(crate) size: Option<(u32, u32)>,
    pub(crate) remote: OwnedFd,
    pub(crate) restore: Option<String>,
}

pub(crate) struct Shared {
    connection: Connection,
    path: OwnedObjectPath,
}

impl Drop for Shared {
    fn drop(&mut self) {
        let closed = SessionProxyBlocking::builder(&self.connection)
            .path(self.path.clone())
            .and_then(|builder| builder.build())
            .and_then(|session| session.close());
        if let Err(error) = closed {
            tracing::debug!(%error, "failed to close screen cast session");
        }
    }
}

pub(crate) fn ask(request_options: &CastRequest) -> Result<CastSession> {
    let connection = Connection::session().map_err(Error::unavailable)?;
    let portal = ScreenCastProxyBlocking::new(&connection).map_err(Error::unavailable)?;
    let offered = portal
        .available_source_types()
        .map_err(Error::unavailable)?;
    let sources = request_options.sources.bits() & offered;
    if sources == 0 {
        return Err(Error::Unavailable(
            "portal offers none of the requested source types".into(),
        ));
    }
    let cursors = portal.available_cursor_modes().unwrap_or(0);

    let session_token = rikka_portal_request::token();
    let created = request(&connection, |token| {
        portal.create_session(HashMap::from([
            ("handle_token", Value::from(token.to_owned())),
            ("session_handle_token", Value::from(session_token.clone())),
        ]))
    })?;
    let path = session_path(&created)?;
    let session = Shared {
        connection: connection.clone(),
        path: path.clone(),
    };

    let mut options = HashMap::from([
        ("types", Value::from(sources)),
        ("multiple", Value::from(false)),
    ]);
    if request_options.persist || request_options.restore.is_some() {
        options.insert("persist_mode", Value::from(PERSIST_UNTIL_REVOKED));
    }
    let cursor = match (request_options.cursor, cursors & EMBEDDED_CURSOR != 0) {
        (true, true) => Some(EMBEDDED_CURSOR),
        (false, _) if cursors & HIDDEN_CURSOR != 0 => Some(HIDDEN_CURSOR),
        _ => None,
    };
    if let Some(cursor) = cursor {
        options.insert("cursor_mode", Value::from(cursor));
    }
    if let Some(restore) = &request_options.restore {
        options.insert("restore_token", Value::from(restore.clone()));
    }
    request(&connection, |token| {
        let mut options = options.clone();
        options.insert("handle_token", Value::from(token.to_owned()));
        portal.select_sources(&path.as_ref(), options)
    })?;

    let started = request(&connection, |token| {
        portal.start(
            &path.as_ref(),
            "",
            HashMap::from([("handle_token", Value::from(token.to_owned()))]),
        )
    })?;
    let (node, kind, size) = first_stream(&started)
        .ok_or_else(|| Error::Malformed("portal response has no streams".into()))?;
    let restore = started
        .get("restore_token")
        .and_then(|token| String::try_from(token.clone()).ok());
    let remote = portal
        .open_pipe_wire_remote(&path.as_ref(), HashMap::new())
        .map(OwnedFd::from)
        .map_err(Error::unavailable)?;
    Ok(CastSession {
        session,
        node,
        kind,
        size,
        remote,
        restore,
    })
}

fn request(
    connection: &Connection,
    call: impl FnOnce(&str) -> zbus::Result<OwnedObjectPath>,
) -> Result<Results> {
    let answer = rikka_portal_request::ask(connection, call).map_err(|error| match error {
        rikka_portal_request::Error::Unreachable(reason) => Error::Unavailable(reason),
        rikka_portal_request::Error::Malformed(reason) => Error::Malformed(reason),
        rikka_portal_request::Error::Silent => {
            Error::Unavailable("portal request ended without a response".into())
        }
    })?;
    match answer {
        Answer::Granted(results) => Ok(results),
        Answer::Cancelled => Err(Error::Cancelled),
        Answer::Ended => Err(Error::Refused),
    }
}

fn session_path(results: &Results) -> Result<OwnedObjectPath> {
    let value = results
        .get("session_handle")
        .ok_or_else(|| Error::Malformed("no session handle".into()))?;
    if let Ok(path) = OwnedObjectPath::try_from(value.clone()) {
        return Ok(path);
    }
    let spelled = String::try_from(value.clone())
        .map_err(|_| Error::Malformed("session_handle is not an object path".into()))?;
    OwnedObjectPath::try_from(spelled).map_err(|error| Error::Malformed(error.to_string()))
}

type Stream = (u32, HashMap<String, OwnedValue>);

type StreamInfo = (u32, SourceKind, Option<(u32, u32)>);

fn first_stream(results: &Results) -> Option<StreamInfo> {
    let streams = results.get("streams")?;
    let streams = <Vec<Stream>>::try_from(streams.clone()).ok()?;
    let (node, properties) = streams.into_iter().next()?;
    let kind = properties
        .get("source_type")
        .and_then(|kind| u32::try_from(kind.clone()).ok())
        .map_or(SourceKind::Monitor, SourceKind::from_bits);
    let size = properties
        .get("size")
        .and_then(|size| <(i32, i32)>::try_from(size.clone()).ok())
        .and_then(|(width, height)| {
            Some((u32::try_from(width).ok()?, u32::try_from(height).ok()?))
        });
    Some((node, kind, size))
}
