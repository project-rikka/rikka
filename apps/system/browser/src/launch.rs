use rikka_app::LaunchRequest;

const NEW_WINDOW: &str = "new-window";
const PRIVATE_WINDOW: &str = "private-window";
const NEW_WINDOW_FLAG: &str = "--new-window";
const PRIVATE_WINDOW_FLAG: &str = "--private-window";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Main,
    NewWindow,
    PrivateWindow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Launch {
    pub(crate) kind: Kind,
    pub(crate) addresses: Vec<String>,
    pub(crate) token: Option<String>,
}

impl Launch {
    pub(crate) fn from_open(open: &LaunchRequest) -> Launch {
        let kind = match open.action() {
            Some(PRIVATE_WINDOW) => Kind::PrivateWindow,
            Some(NEW_WINDOW) => Kind::NewWindow,
            _ if open.has_flag(PRIVATE_WINDOW_FLAG) => Kind::PrivateWindow,
            _ if open.has_flag(NEW_WINDOW_FLAG) => Kind::NewWindow,
            _ => Kind::Main,
        };
        Launch {
            kind,
            addresses: open.uris().to_vec(),
            token: open.token().map(str::to_owned),
        }
    }
}
