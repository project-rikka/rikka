use std::collections::BTreeMap;

use rikka_gecko_protocol::WindowId;
use rikka_gecko_protocol::geckoview::navigation::{LoadFlags, LoadUri};

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Load {
    uri: String,
    flags: LoadFlags,
    headers: BTreeMap<String, Option<String>>,
    referrer: Option<Referrer>,
    typed: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Referrer {
    Uri(String),
    Session(WindowId),
}

impl Load {
    pub fn uri(uri: impl Into<String>) -> Load {
        Load {
            uri: uri.into(),
            flags: LoadFlags::NONE,
            headers: BTreeMap::new(),
            referrer: None,
            typed: None,
        }
    }

    pub fn html(document: impl AsRef<str>) -> Load {
        Load::uri(crate::uris::html(document.as_ref())).flags(LoadFlags::FORCE_ALLOW_DATA_URI)
    }

    pub fn flags(mut self, flags: LoadFlags) -> Load {
        self.flags |= flags;
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Load {
        self.headers.insert(name.into(), Some(value.into()));
        self
    }

    pub fn without_header(mut self, name: impl Into<String>) -> Load {
        self.headers.insert(name.into(), None);
        self
    }

    pub fn referrer(mut self, uri: impl Into<String>) -> Load {
        self.referrer = Some(Referrer::Uri(uri.into()));
        self
    }

    pub fn referrer_session(mut self, session: &crate::session::Session) -> Load {
        self.referrer = Some(Referrer::Session(session.window().clone()));
        self
    }

    pub fn typed(mut self, input: impl Into<String>) -> Load {
        self.typed = Some(input.into());
        self
    }

    pub(crate) fn wire(self) -> LoadUri {
        let mut load = LoadUri::new(self.uri);
        load.flags = self.flags;
        if !self.headers.is_empty() {
            load.headers = Some(self.headers);
        }
        match self.referrer {
            Some(Referrer::Uri(uri)) => load.referrer_uri = Some(uri),
            Some(Referrer::Session(window)) => load.referrer_session_id = Some(window),
            None => {}
        }
        load.original_input = self.typed;
        load
    }
}

impl From<&str> for Load {
    fn from(uri: &str) -> Load {
        Load::uri(uri)
    }
}

impl From<String> for Load {
    fn from(uri: String) -> Load {
        Load::uri(uri)
    }
}
