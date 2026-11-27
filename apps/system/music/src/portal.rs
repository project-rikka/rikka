use std::future::Future;

use music_core::{Cookie, WebSignIn};
use rikka_web::{NewSessionRequest, Session, SignInPortal};

#[derive(Debug)]
pub(crate) struct Portal {
    request: WebSignIn,
    portal: SignInPortal,
}

impl Portal {
    pub(crate) fn open(request: WebSignIn) -> Result<Portal, String> {
        let portal = SignInPortal::open(request.url(), &[]).map_err(|error| error.to_string())?;
        Ok(Portal { request, portal })
    }

    pub(crate) fn session(&self) -> &Session {
        self.portal.session()
    }

    pub(crate) fn follow(&self, opening: NewSessionRequest) {
        self.portal.follow(opening);
    }

    pub(crate) fn is_on_landing_host(&self) -> bool {
        let url = self.portal.session().page().url;
        self.request.arrived(&url) && url.starts_with(self.request.cookies_of())
    }

    pub(crate) fn cookies(&self) -> impl Future<Output = Result<Vec<Cookie>, String>> + use<> {
        let cookies = self.portal.cookies(self.request.cookies_of());
        async move {
            Ok(cookies
                .await
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|cookie| Cookie {
                    name: cookie.name,
                    value: cookie.value,
                })
                .collect())
        }
    }

    pub(crate) fn has_proof_cookie(&self, cookies: &[Cookie]) -> bool {
        self.request.has_proof_cookie(cookies)
    }

    pub(crate) fn finish(self, cookies: Vec<Cookie>) {
        self.request.answer(cookies);
    }
}
