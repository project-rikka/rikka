use rikka_seam::net::{self, SocketClient};
use rikka_seam::{CAPABILITIES, Manifest, Reply, Request, SEAM_CAPABILITY, SeamClient};
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

#[derive(Debug)]
pub(crate) struct CapabilitySocket(SocketClient);

impl CapabilitySocket {
    pub(crate) fn connect(capability: &str) -> Result<CapabilitySocket> {
        SocketClient::for_capability(capability)
            .map(CapabilitySocket)
            .map_err(Error::NoProvider)
    }

    pub(crate) fn client(&self) -> SeamClient {
        self.0.client()
    }
}

#[derive(Debug)]
pub(crate) struct Session {
    sockets: Vec<SocketClient>,
}

impl Session {
    pub(crate) fn connect() -> Result<Session> {
        let mut sockets = Vec::new();
        let mut failures = Vec::new();
        for path in net::session_sockets() {
            match SocketClient::connect(&path) {
                Ok(socket) => sockets.push(socket),
                Err(error) => failures.push(format!("{}: {error}", path.display())),
            }
        }
        if sockets.is_empty() {
            return Err(Error::NoSession(match failures.is_empty() {
                true => "no socket".to_owned(),
                false => failures.join("; "),
            }));
        }
        Ok(Session { sockets })
    }

    pub(crate) fn manifests(&self) -> Vec<Manifest> {
        let replies: Vec<Reply> = self
            .clients()
            .map(|seam| seam.call(Request::new(SEAM_CAPABILITY, CAPABILITIES)))
            .collect();
        let mut listed: Vec<Manifest> = Vec::new();
        for reply in replies {
            let provided = match reply.decode::<Vec<Manifest>>() {
                Ok(provided) => provided,
                Err(error) => {
                    eprintln!("warning: cannot list capabilities: {error}");
                    continue;
                }
            };
            for manifest in provided {
                if !listed
                    .iter()
                    .any(|known| known.capability() == manifest.capability())
                {
                    listed.push(manifest);
                }
            }
        }
        listed
    }

    pub(crate) fn clients(&self) -> impl Iterator<Item = SeamClient> + '_ {
        self.sockets.iter().map(SocketClient::client)
    }
}

pub(crate) trait ReplyExt {
    fn decode<T: DeserializeOwned>(self) -> Result<T>;

    fn finish(self) -> Result<()>;
}

impl ReplyExt for Reply {
    fn decode<T: DeserializeOwned>(self) -> Result<T> {
        Ok(self.blocking_wait()?.decode()?)
    }

    fn finish(self) -> Result<()> {
        self.blocking_wait()?;
        Ok(())
    }
}
