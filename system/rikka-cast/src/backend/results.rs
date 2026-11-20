use serde::Serialize;
use zbus::zvariant::{OwnedValue, SerializeDict, Type};

use crate::backend::cast::CastStream;
use crate::backend::options;
use crate::backend::restore;
use crate::protocol::{CastSource, Persist};

#[derive(Debug, SerializeDict, Type)]
#[zvariant(signature = "a{sv}")]
pub(crate) struct CreateSessionResults {
    session_id: String,
}

impl CreateSessionResults {
    pub(crate) fn new(session: &str) -> CreateSessionResults {
        CreateSessionResults {
            session_id: session.to_owned(),
        }
    }
}

#[derive(Debug, Default, SerializeDict, Type)]
#[zvariant(signature = "a{sv}")]
pub(crate) struct EmptyResults {}

#[derive(Debug, Serialize, Type)]
pub(crate) struct Stream(u32, StreamProps);

#[derive(Debug, Default, SerializeDict, Type)]
#[zvariant(signature = "a{sv}")]
struct StreamProps {
    position: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
    source_type: u32,
    #[zvariant(rename = "pipewire-serial")]
    pipewire_serial: u64,
}

#[derive(Debug, Default, SerializeDict, Type)]
#[zvariant(signature = "a{sv}")]
pub(crate) struct StartResults {
    streams: Option<Vec<Stream>>,
    persist_mode: Option<u32>,
    restore_data: Option<(String, u32, OwnedValue)>,
}

impl StartResults {
    pub(crate) fn new(streams: &[CastStream], persist: Persist) -> StartResults {
        let sources: Vec<CastSource> = streams.iter().map(|stream| stream.source.clone()).collect();
        StartResults {
            streams: Some(
                streams
                    .iter()
                    .map(|stream| {
                        Stream(
                            stream.node,
                            StreamProps {
                                position: stream.position,
                                size: Some(stream.size),
                                source_type: options::source_type(stream.source.kind),
                                pipewire_serial: stream.serial,
                            },
                        )
                    })
                    .collect(),
            ),
            persist_mode: Some(options::persist_mode(persist)),
            restore_data: match persist {
                Persist::No => None,
                Persist::WhileRunning | Persist::UntilRevoked => Some(restore::write(&sources)),
            },
        }
    }
}
