use std::future::Future;

use rikka_gecko_protocol::rikka::storage::Cookie;

use crate::error::Result;
use crate::process::profile::Profile;
use crate::runtime::Runtime;
use crate::session::{NewSessionRequest, Session, SessionSettings, UserContent};

pub struct SignInPortal {
    session: Session,
}

impl SignInPortal {
    pub fn open(url: &str, content: UserContent) -> Result<SignInPortal> {
        let runtime = Runtime::builder().profile(Profile::Ephemeral).start()?;
        let session = runtime.session(SessionSettings::new().private(true))?;
        session.set_user_content(content);
        session.load(url);
        Ok(SignInPortal { session })
    }

    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn follow(&self, opening: NewSessionRequest) {
        let uri = opening.uri.clone();
        opening.deny();
        self.session.load(uri);
    }

    pub fn cookies(&self, site: &str) -> impl Future<Output = Result<Vec<Cookie>>> + use<> {
        let session = self.session.clone();
        let site = site.to_owned();
        async move {
            session
                .runtime()
                .storage()
                .cookies(site, &session.partition())
                .await
        }
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
