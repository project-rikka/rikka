use http::{HeaderMap, HeaderName, HeaderValue};

use crate::auth::Auth;
use crate::cancel::CancelToken;
use crate::error::{Error, Result};
use crate::headers;
use crate::request::RequestBuilder;

#[derive(Debug, Default)]
pub(crate) struct RequestOptions {
    headers: Vec<(HeaderName, HeaderValue)>,
    auth: Option<Auth>,
    cancel: Option<CancelToken>,
    error: Option<Error>,
}

impl RequestOptions {
    pub(crate) fn header<K, V>(&mut self, name: K, value: V)
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        match headers::pair(name, value) {
            Ok(pair) => self.headers.push(pair),
            Err(error) => self.fail(error),
        }
    }

    pub(crate) fn auth(&mut self, auth: Auth) {
        self.auth = Some(auth);
    }

    pub(crate) fn set_cancel(&mut self, token: CancelToken) {
        self.cancel = Some(token);
    }

    pub(crate) fn cancel(&self) -> Option<&CancelToken> {
        self.cancel.as_ref()
    }

    pub(crate) fn cancel_or_default(&mut self) -> CancelToken {
        self.cancel.get_or_insert_with(CancelToken::default).clone()
    }

    pub(crate) fn apply(&self, mut request: RequestBuilder) -> RequestBuilder {
        for (name, value) in &self.headers {
            request = request.header(name.clone(), value.clone());
        }
        if let Some(auth) = &self.auth {
            request = request.auth(auth.clone());
        }
        if let Some(token) = &self.cancel {
            request = request.cancel_token(token.clone());
        }
        request
    }

    pub(crate) fn apply_to_headers(&self, headers: &mut HeaderMap) -> Result<()> {
        for (name, value) in &self.headers {
            headers.insert(name.clone(), value.clone());
        }
        if let Some(auth) = &self.auth {
            let (name, value) = auth.header()?;
            headers.insert(name, value);
        }
        Ok(())
    }

    pub(crate) fn check(&mut self) -> Result<()> {
        self.error.take().map_or(Ok(()), Err)
    }

    fn fail(&mut self, error: Error) {
        if self.error.is_none() {
            self.error = Some(error);
        }
    }
}
