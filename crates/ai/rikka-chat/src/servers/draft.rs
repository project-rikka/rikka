use std::collections::BTreeMap;

use rikka_mcp::{Http, Oauth, Server, ServerConfigError, Stdio, Transport};

use crate::prelude::*;
use crate::servers::trial::Trial;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Kind {
    #[default]
    Command,
    Address,
}

impl Kind {
    pub(crate) const ALL: [Kind; 2] = [Kind::Command, Kind::Address];

    pub(crate) fn at(self) -> usize {
        match self {
            Kind::Command => 0,
            Kind::Address => 1,
        }
    }

    pub(crate) fn title(self) -> Label {
        match self {
            Kind::Command => tr!("chat.servers.command"),
            Kind::Address => tr!("chat.servers.address"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    ServerConfigError(ServerConfigError),
    Unclosed,
    Keyless,
    Repeated(String),
    Port,
    Taken,
}

pub(crate) struct Pair {
    key: TextDocument,
    value: TextDocument,
    revealed: bool,
}

impl Pair {
    fn new(key: &str, value: &str) -> Pair {
        Pair {
            key: TextDocument::new(key),
            value: TextDocument::new(value),
            revealed: false,
        }
    }

    pub(crate) fn key(&self) -> &TextDocument {
        &self.key
    }

    pub(crate) fn value(&self) -> &TextDocument {
        &self.value
    }

    pub(crate) fn is_shown(&self) -> bool {
        self.revealed
    }
}

pub(crate) struct Signing {
    client: TextDocument,
    secret: TextDocument,
    port: TextDocument,
    revealed: bool,
}

impl Signing {
    fn from_oauth(oauth: Option<&Oauth>) -> Signing {
        Signing {
            client: TextDocument::new(oauth.and_then(Oauth::client_id).unwrap_or_default()),
            secret: TextDocument::new(oauth.and_then(Oauth::client_secret).unwrap_or_default()),
            port: TextDocument::new(
                oauth
                    .and_then(Oauth::callback_port)
                    .map(|port| port.to_string())
                    .unwrap_or_default(),
            ),
            revealed: false,
        }
    }

    pub(crate) fn client(&self) -> &TextDocument {
        &self.client
    }

    pub(crate) fn secret(&self) -> &TextDocument {
        &self.secret
    }

    pub(crate) fn port(&self) -> &TextDocument {
        &self.port
    }

    pub(crate) fn is_shown(&self) -> bool {
        self.revealed
    }

    fn oauth(&self) -> Result<Oauth, Fault> {
        let mut oauth = Oauth::new()
            .client(self.client.text().trim())
            .secret(self.secret.text().trim());
        let port = self.port.text();
        let port = port.trim();
        if !port.is_empty() {
            oauth = oauth.port(port.parse::<u16>().map_err(|_| Fault::Port)?);
        }
        Ok(oauth)
    }
}

pub(crate) struct Draft {
    was: Option<Server>,
    name: TextDocument,
    kind: Kind,
    line: TextDocument,
    folder: TextDocument,
    url: TextDocument,
    variables: Vec<Pair>,
    headers: Vec<Pair>,
    signing: Signing,
    advanced: bool,
    trial: Trial,
}

impl Draft {
    pub(crate) fn blank() -> Draft {
        Draft {
            was: None,
            name: TextDocument::new(""),
            kind: Kind::default(),
            line: TextDocument::new(""),
            folder: TextDocument::new(""),
            url: TextDocument::new(""),
            variables: Vec::new(),
            headers: Vec::new(),
            signing: Signing::from_oauth(None),
            advanced: false,
            trial: Trial::Idle,
        }
    }

    pub(crate) fn from_server(server: &Server) -> Draft {
        let mut draft = Draft {
            was: Some(server.clone()),
            name: TextDocument::new(server.name()),
            ..Draft::blank()
        };
        match server.transport() {
            Transport::Stdio(stdio) => {
                draft.line = TextDocument::new(stdio.command_line());
                if let Some(folder) = stdio.folder() {
                    draft.folder = TextDocument::new(folder.display().to_string());
                }
                draft.variables = paired(stdio.variables());
            }
            Transport::Http(http) => {
                draft.kind = Kind::Address;
                draft.url = TextDocument::new(http.url());
                draft.headers = paired(http.headers());
                draft.signing = Signing::from_oauth(http.oauth());
                draft.advanced = http.oauth().is_some();
            }
        }
        draft
    }

    pub(crate) fn was(&self) -> Option<&str> {
        self.was.as_ref().map(Server::name)
    }

    pub(crate) fn name(&self) -> &TextDocument {
        &self.name
    }

    pub(crate) fn kind(&self) -> Kind {
        self.kind
    }

    pub(crate) fn line(&self) -> &TextDocument {
        &self.line
    }

    pub(crate) fn folder(&self) -> &TextDocument {
        &self.folder
    }

    pub(crate) fn url(&self) -> &TextDocument {
        &self.url
    }

    pub(crate) fn signing(&self) -> &Signing {
        &self.signing
    }

    pub(crate) fn is_advanced(&self) -> bool {
        self.advanced
    }

    pub(crate) fn pairs(&self) -> &[Pair] {
        match self.kind {
            Kind::Command => &self.variables,
            Kind::Address => &self.headers,
        }
    }

    pub(crate) fn trial(&self) -> &Trial {
        &self.trial
    }

    pub(crate) fn choose(&mut self, kind: Kind) {
        self.kind = kind;
        self.changed();
    }

    pub(crate) fn locate(&mut self, folder: &str) {
        self.folder.set_text(folder);
        self.changed();
    }

    pub(crate) fn add_pair(&mut self) {
        self.pairs_mut().push(Pair::new("", ""));
    }

    pub(crate) fn drop_pair(&mut self, at: usize) {
        let pairs = self.pairs_mut();
        if at < pairs.len() {
            pairs.remove(at);
        }
        self.changed();
    }

    pub(crate) fn toggle_pair(&mut self, at: usize) {
        if let Some(pair) = self.pairs_mut().get_mut(at) {
            pair.revealed = !pair.revealed;
        }
    }

    pub(crate) fn toggle_secret(&mut self) {
        self.signing.revealed = !self.signing.revealed;
    }

    pub(crate) fn unfold_advanced(&mut self) {
        self.advanced = !self.advanced;
    }

    pub(crate) fn changed(&mut self) {
        self.trial = Trial::Idle;
    }

    pub(crate) fn tried(&mut self, trial: Trial) {
        self.trial = trial;
    }

    pub(crate) fn server(&self) -> Result<Server, Fault> {
        let transport: Transport = match self.kind {
            Kind::Command => {
                let mut stdio = Stdio::parse(&self.line.text())
                    .map_err(|_| Fault::Unclosed)?
                    .env(gathered(&self.variables)?);
                let folder = self.folder.text();
                if !folder.trim().is_empty() {
                    stdio = stdio.cwd(folder.trim());
                }
                stdio.into()
            }
            Kind::Address => Http::new(self.url.text().trim())
                .with_headers(gathered(&self.headers)?)
                .with_oauth(self.signing.oauth()?)
                .into(),
        };

        let mut server = Server::new(self.name.text().trim(), transport);
        if let Some(was) = &self.was {
            server = server
                .with_disabled(was.is_disabled())
                .with_extra(was.extra().clone());
        }
        match server.config_error() {
            Some(nonsense) => Err(Fault::ServerConfigError(nonsense)),
            None => Ok(server),
        }
    }

    fn pairs_mut(&mut self) -> &mut Vec<Pair> {
        match self.kind {
            Kind::Command => &mut self.variables,
            Kind::Address => &mut self.headers,
        }
    }
}

impl std::fmt::Debug for Draft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Draft")
            .field("was", &self.was())
            .field("kind", &self.kind)
            .field("trial", &self.trial)
            .finish_non_exhaustive()
    }
}

fn paired(values: &BTreeMap<String, String>) -> Vec<Pair> {
    values
        .iter()
        .map(|(key, value)| Pair::new(key, value))
        .collect()
}

fn gathered(pairs: &[Pair]) -> Result<BTreeMap<String, String>, Fault> {
    let mut gathered = BTreeMap::new();
    for pair in pairs {
        let key = pair.key.text().trim().to_owned();
        let value = pair.value.text().trim().to_owned();
        if key.is_empty() {
            match value.is_empty() {
                true => continue,
                false => return Err(Fault::Keyless),
            }
        }
        if gathered.contains_key(&key) {
            return Err(Fault::Repeated(key));
        }
        gathered.insert(key, value);
    }
    Ok(gathered)
}
