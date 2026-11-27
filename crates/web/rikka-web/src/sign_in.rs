use std::future::Future;

use crate::api::Cookie;
use crate::error::Result;
use crate::opening::NewSessionRequest;
use crate::process::profile::Profile;
use crate::runtime::Runtime;
use crate::script::UserScript;
use crate::session::{Session, SessionSettings};

pub struct SignInPortal {
    session: Session,
}

impl SignInPortal {
    pub fn open(url: &str, scripts: &[UserScript]) -> Result<SignInPortal> {
        let runtime = Runtime::builder().profile(Profile::Ephemeral).start()?;
        for script in scripts {
            runtime.register(script);
        }
        let session = runtime.session(SessionSettings::new().url(url))?;
        Ok(SignInPortal { session })
    }

    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn follow(&self, opening: NewSessionRequest) {
        let url = opening.url.clone();
        opening.deny();
        self.session.load(url);
    }

    pub fn cookies(&self, url: &str) -> impl Future<Output = Result<Vec<Cookie>>> + Send + use<> {
        self.session.runtime().storage().cookies(url)
    }
}

impl std::fmt::Debug for SignInPortal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SignInPortal")
            .field("session", &self.session.id())
            .finish_non_exhaustive()
    }
}
