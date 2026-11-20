use std::path::PathBuf;

use crate::error::{Error, Result};

const FOLDER: &str = "remote";
const CERTIFICATE: &str = "certificate.pem";
const DEVICES: &str = "devices";

#[derive(Clone, Debug)]
pub(crate) struct Identity {
    pub(crate) user: String,
    pub(crate) host: String,
    pub(crate) state: PathBuf,
}

impl Identity {
    pub(crate) fn current() -> Result<Identity> {
        let user = rikka_dirs::passwd::current().ok_or(Error::NoUser)?.name;
        let host = rustix::system::uname()
            .nodename()
            .to_string_lossy()
            .into_owned();
        let state = rikka_dirs::Paths::resolve()?.state_dir().join(FOLDER);
        Ok(Identity { user, host, state })
    }

    pub(crate) fn certificate(&self) -> PathBuf {
        self.state.join(CERTIFICATE)
    }

    pub(crate) fn devices(&self) -> PathBuf {
        self.state.join(DEVICES)
    }
}
