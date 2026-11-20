use std::path::PathBuf;

use rikka_seam::SeamClient;
use rikka_seam::net::SocketClient;
use rikka_session::replay::{CAPABILITY, Draft, Keep, ReplayClient, ReplayStatus, Save, Saved};
use rikka_ui::tr;
use serde::de::DeserializeOwned;
use shell_notifications::Announcement;

use crate::action_request;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("instant replay unavailable: {0}")]
    Unreachable(std::io::Error),

    #[error("{0}")]
    Refused(String),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

fn client() -> Result<(SocketClient, ReplayClient)> {
    let connection = SocketClient::for_capability(CAPABILITY).map_err(Error::Unreachable)?;
    let replay = ReplayClient::new(connection.client());
    Ok((connection, replay))
}

fn answered<T: DeserializeOwned>(reply: rikka_seam::Reply) -> Result<T> {
    reply
        .blocking_wait()
        .and_then(|answer| answer.decode())
        .map_err(|error| Error::Refused(error.to_string()))
}

pub(crate) fn buffer() -> Result<ReplayStatus> {
    let (_connection, replay) = client()?;
    answered(replay.status())
}

pub(crate) fn save(save: &Save) -> Result<Saved> {
    let (_connection, replay) = client()?;
    answered(replay.save(save))
}

pub(crate) fn draft(subject: &str) -> Result<Draft> {
    let (_connection, replay) = client()?;
    answered(replay.draft(subject))
}

pub(crate) fn keep(keep: &Keep) -> Result<Saved> {
    let (_connection, replay) = client()?;
    answered(replay.keep(keep))
}

pub(crate) fn discard(draft: PathBuf) {
    let discarded = client().and_then(|(_connection, replay)| {
        replay
            .discard(draft)
            .blocking_wait()
            .map(drop)
            .map_err(|error| Error::Refused(error.to_string()))
    });
    if let Err(error) = discarded {
        tracing::debug!("failed to remove replay draft: {error}");
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Strings {
    name: String,
    saved: String,
    failed: String,
}

impl Strings {
    #[must_use]
    pub(crate) fn now() -> Strings {
        Strings {
            name: tr!("shell.replay.name").to_string(),
            saved: tr!("shell.replay.saved").to_string(),
            failed: tr!("shell.replay.failed").to_string(),
        }
    }

    pub(crate) fn notify_saved(&self, saved: &Saved, seam: &SeamClient) {
        Announcement::new(&self.name, &self.saved)
            .file(&saved.file)
            .action_request(action_request(&saved.file))
            .send(seam);
    }

    pub(crate) fn notify_failed(&self, reason: &str, seam: &SeamClient) {
        Announcement::new(&self.name, &self.failed)
            .body(reason)
            .urgent()
            .send(seam);
    }
}
