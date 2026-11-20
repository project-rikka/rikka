use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rikka_fs::Replace;

use crate::error::{Error, Result};
use crate::server::Server;
use crate::wire::Wire;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Document {
    #[serde(rename = "mcpServers", alias = "servers", default)]
    servers: BTreeMap<String, Wire>,
    #[serde(flatten)]
    rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "Vec<Server>", into = "Vec<Server>")]
pub struct Catalog {
    servers: Vec<Server>,
    rest: serde_json::Map<String, serde_json::Value>,
}

impl From<Vec<Server>> for Catalog {
    fn from(servers: Vec<Server>) -> Catalog {
        Catalog::from_servers(servers)
    }
}

impl From<Catalog> for Vec<Server> {
    fn from(catalog: Catalog) -> Vec<Server> {
        catalog.servers
    }
}

impl Catalog {
    pub const FILE: &'static str = ".mcp.json";

    #[must_use]
    pub fn new() -> Catalog {
        Catalog::default()
    }

    #[must_use]
    pub fn from_servers(servers: impl IntoIterator<Item = Server>) -> Catalog {
        let mut catalog = Catalog::new();
        for server in servers {
            catalog.insert(server);
        }
        catalog
    }

    #[must_use]
    pub fn merge(catalogs: impl IntoIterator<Item = Catalog>) -> Catalog {
        Catalog::from_servers(catalogs.into_iter().flat_map(Catalog::into_servers))
    }

    #[must_use]
    pub fn path_in(project: &Path) -> PathBuf {
        project.join(Catalog::FILE)
    }

    pub fn read(path: &Path) -> Result<Catalog> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Catalog::new()),
            Err(error) => {
                return Err(Error::Unreadable {
                    path: path.display().to_string(),
                    reason: error.to_string(),
                });
            }
        };

        Catalog::parse(&text, &path.display().to_string())
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let unwritable = |error: &dyn std::fmt::Display| Error::Unwritable {
            path: path.display().to_string(),
            reason: error.to_string(),
        };

        if self.servers.is_empty() && self.rest.is_empty() {
            return match std::fs::remove_file(path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    Err(unwritable(&error))
                }
                _ => Ok(()),
            };
        }

        Replace::new(path)
            .private()
            .write(self.to_json())
            .map_err(|error| unwritable(&error))
    }

    pub fn from_json(text: &str) -> Result<Catalog> {
        Catalog::parse(text, "input")
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let document = Document {
            servers: self
                .servers
                .iter()
                .map(|server| {
                    let name = server.name().to_owned();
                    let wire = Wire {
                        name: None,
                        ..Wire::from(server.clone())
                    };
                    (name, wire)
                })
                .collect(),
            rest: self.rest.clone(),
        };

        let mut encoded = serde_json::to_string_pretty(&document)
            .expect("string-keyed JSON documents always serialize");
        encoded.push('\n');
        encoded
    }

    #[must_use]
    pub fn servers(&self) -> &[Server] {
        &self.servers
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Server> {
        self.servers.iter().find(|server| server.name() == name)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    pub fn insert(&mut self, server: Server) {
        match self.position(server.name()) {
            Ok(at) => self.servers[at] = server,
            Err(at) => self.servers.insert(at, server),
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        match self.position(name) {
            Ok(at) => {
                self.servers.remove(at);
                true
            }
            Err(_) => false,
        }
    }

    #[must_use]
    pub fn into_servers(self) -> Vec<Server> {
        self.servers
    }

    fn position(&self, name: &str) -> std::result::Result<usize, usize> {
        self.servers
            .binary_search_by(|server| server.name().cmp(name))
    }

    fn parse(text: &str, source: &str) -> Result<Catalog> {
        if text.trim().is_empty() {
            return Ok(Catalog::new());
        }

        let document: Document = serde_json::from_str(text).map_err(|error| Error::Malformed {
            document: source.to_owned(),
            reason: error.to_string(),
        })?;

        let mut catalog =
            Catalog::from_servers(document.servers.into_iter().map(|(name, wire)| {
                Server::new(name, wire.transport())
                    .with_disabled(wire.disabled)
                    .with_extra(wire.rest)
            }));
        catalog.rest = document.rest;
        Ok(catalog)
    }
}
