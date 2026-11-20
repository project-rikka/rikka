use crate::request::Action;
use crate::snapshot::Snapshot;

#[derive(Clone, Debug, Default)]
pub struct Secrets {
    pub passphrase: Option<String>,
    pub password: Option<String>,
    pub private_key_password: Option<String>,
}

impl Secrets {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.passphrase.is_none() && self.password.is_none() && self.private_key_password.is_none()
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    Changed(Box<Snapshot>),
    Answered {
        action: Action,
        error: Option<String>,
    },
    Revealed {
        uuid: String,
        secrets: Secrets,
    },
}
