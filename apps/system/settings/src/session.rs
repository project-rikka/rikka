use rikka_seam::SeamClient;
use rikka_seam::net::SocketClient;
use rikka_settings::{Error, Result};

pub(crate) struct Session(Result<SocketClient, String>);

impl Session {
    pub(crate) fn connect() -> Self {
        Self(SocketClient::from_env().map_err(|reason| reason.to_string()))
    }

    pub(crate) fn seam(&self, domain: &str) -> Result<SeamClient> {
        match &self.0 {
            Ok(connection) => Ok(connection.client()),
            Err(reason) => Err(Error::Session {
                code: 0,
                message: format!("there is no session to ask about `{domain}`: {reason}"),
            }),
        }
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Session").field(&self.0.is_ok()).finish()
    }
}
