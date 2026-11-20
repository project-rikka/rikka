mod network;
mod page;
mod view;

pub(crate) use page::document;
pub(crate) use view::view;

use rikka_web::{ErrorCode, Hang, LoadError};

use browser_pages::FailureKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ended {
    Crashed,
    Killed,
    EngineExited,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Problem {
    Certificate {
        host: String,
    },
    HttpsOnly {
        host: String,
        http: String,
    },
    Network {
        uri: String,
        host: String,
        kind: FailureKind,
    },
    Gone(Ended),
}

#[derive(Debug, Default)]
pub(crate) struct Failure {
    problem: Option<Problem>,
    alternate: Option<String>,
    hang: Option<Hang>,
}

pub(crate) fn problem_of(error: &LoadError) -> Option<Problem> {
    let uri = error.uri.clone()?;
    let host = shown_host(&uri);
    Some(match error.error.code {
        ErrorCode::HttpsOnly => Problem::HttpsOnly {
            http: http_of(&uri),
            host: browser_pages::web_host(&uri).unwrap_or(host),
        },
        ErrorCode::SecuritySsl | ErrorCode::SecurityBadCert | ErrorCode::BadHstsCert => {
            Problem::Certificate { host }
        }
        code => Problem::Network {
            uri,
            host,
            kind: FailureKind::from_code(code),
        },
    })
}

fn http_of(uri: &str) -> String {
    match uri.strip_prefix("https://") {
        Some(rest) => format!("http://{rest}"),
        None => uri.to_owned(),
    }
}

fn shown_host(uri: &str) -> String {
    let shown = crate::address::display_address(uri);
    match shown.host.is_empty() {
        true => shown.rest,
        false => shown.host,
    }
}

impl Problem {
    pub(crate) fn title(&self) -> rikka_app::prelude::Label {
        view::title(self)
    }
}

impl Failure {
    pub(crate) fn problem(&self) -> Option<&Problem> {
        self.problem.as_ref()
    }

    pub(crate) fn failed(&mut self, uri: Option<String>, problem: Problem) {
        self.alternate = uri;
        self.problem = Some(problem);
    }

    pub(crate) fn gone(&mut self, ended: Ended) {
        self.problem = Some(Problem::Gone(ended));
        self.alternate = None;
        self.hang = None;
    }

    pub(crate) fn started(&mut self, uri: &str) {
        let shown_instead = uri.starts_with("data:") || uri.starts_with("about:");
        if shown_instead || self.alternate.as_deref() == Some(uri) {
            return;
        }
        self.clear();
    }

    pub(crate) fn clear(&mut self) {
        self.problem = None;
        self.alternate = None;
    }

    pub(crate) fn hung(&self) -> bool {
        self.hang.is_some()
    }

    pub(crate) fn hang(&mut self, hang: Hang) {
        self.hang = Some(hang);
    }

    pub(crate) fn take_hang(&mut self) -> Option<Hang> {
        self.hang.take()
    }
}
