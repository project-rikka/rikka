use std::fs;
use std::io;
use std::path::Path;

use rikka_remote_link::Pin;
use rikka_remote_wire::DEFAULT_PORT;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const LONGEST_NAME: usize = 253;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    pub address: String,
    pub name: String,
    #[serde(serialize_with = "pin_to_hex", deserialize_with = "pin_from_hex")]
    pub pin: Option<Pin>,
    pub used: u64,
}

fn pin_to_hex<S: Serializer>(pin: &Option<Pin>, serializer: S) -> Result<S::Ok, S::Error> {
    pin.map(Pin::to_hex).serialize(serializer)
}

fn pin_from_hex<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Pin>, D::Error> {
    match Option::<String>::deserialize(deserializer)? {
        Some(text) => Pin::from_hex(&text)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("{text} is not a fingerprint"))),
        None => Ok(None),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    #[error("no address was given")]
    Empty,
    #[error("{0} is not a port number")]
    Port(String),
    #[error("{0} is not a host name")]
    Name(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub port: u16,
}

impl Target {
    pub fn parse(text: &str) -> Result<Target, AddressError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(AddressError::Empty);
        }
        let (host, port) = match text.strip_prefix('[') {
            Some(bracketed) => {
                let (host, rest) = bracketed
                    .split_once(']')
                    .ok_or_else(|| AddressError::Name(text.to_owned()))?;
                let port = match rest.strip_prefix(':') {
                    Some(port) => Some(port),
                    None if rest.is_empty() => None,
                    None => return Err(AddressError::Name(text.to_owned())),
                };
                (host, port)
            }
            None => match text.rsplit_once(':') {
                Some((host, port)) if !host.contains(':') => (host, Some(port)),
                _ => (text, None),
            },
        };
        let port = match port {
            Some(port) => port
                .parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or_else(|| AddressError::Port(port.to_owned()))?,
            None => DEFAULT_PORT,
        };
        let valid = !host.is_empty()
            && host.len() <= LONGEST_NAME
            && host
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-._:".contains(character));
        if !valid {
            return Err(AddressError::Name(host.to_owned()));
        }
        Ok(Target {
            host: host.to_owned(),
            port,
        })
    }

    #[must_use]
    pub fn address(&self) -> String {
        match self.host.contains(':') {
            true => format!("[{}]:{}", self.host, self.port),
            false => format!("{}:{}", self.host, self.port),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hosts {
    hosts: Vec<Host>,
}

impl Hosts {
    pub fn load(path: &Path) -> io::Result<Hosts> {
        match fs::read(path) {
            Ok(bytes) => {
                let hosts: Vec<Host> = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                Ok(Hosts { hosts })
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Hosts::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        rikka_fs::Replace::new(path)
            .private()
            .write(serde_json::to_vec_pretty(&self.hosts).map_err(io::Error::other)?)
    }

    #[must_use]
    pub fn list(&self) -> Vec<&Host> {
        let mut listed: Vec<&Host> = self.hosts.iter().collect();
        listed.sort_by(|a, b| b.used.cmp(&a.used).then_with(|| a.name.cmp(&b.name)));
        listed
    }

    #[must_use]
    pub fn get(&self, address: &str) -> Option<&Host> {
        self.hosts.iter().find(|host| host.address == address)
    }

    pub fn remember(&mut self, host: Host) {
        match self
            .hosts
            .iter_mut()
            .find(|known| known.address == host.address)
        {
            Some(known) => *known = host,
            None => self.hosts.push(host),
        }
    }

    pub fn forget(&mut self, address: &str) -> Option<Host> {
        let index = self.hosts.iter().position(|host| host.address == address)?;
        Some(self.hosts.remove(index))
    }
}
