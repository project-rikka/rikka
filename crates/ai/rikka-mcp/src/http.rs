use std::collections::BTreeMap;

use crate::oauth::Oauth;

#[must_use]
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Http {
    url: String,
    headers: BTreeMap<String, String>,
    oauth: Option<Oauth>,
}

impl Http {
    pub fn new(url: impl Into<String>) -> Http {
        Http {
            url: url.into(),
            headers: BTreeMap::new(),
            oauth: None,
        }
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    pub fn with_headers(mut self, headers: impl IntoIterator<Item = (String, String)>) -> Self {
        self.headers.extend(headers);
        self
    }

    pub fn with_oauth(mut self, oauth: Oauth) -> Self {
        self.oauth = Some(oauth).filter(|oauth| !oauth.is_empty());
        self
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    #[must_use]
    pub fn oauth(&self) -> Option<&Oauth> {
        self.oauth.as_ref()
    }
}

impl std::fmt::Debug for Http {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Http")
            .field("url", &self.url)
            .field("headers", &self.headers.keys().collect::<Vec<_>>())
            .field("oauth", &self.oauth)
            .finish()
    }
}
