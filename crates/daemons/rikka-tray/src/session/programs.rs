use std::collections::HashMap;
use std::path::Path;

use zbus::blocking::Connection;
use zbus::blocking::fdo::DBusProxy;
use zbus::names::BusName;

#[derive(Debug, Default)]
pub(crate) struct Programs(HashMap<String, String>);

impl Programs {
    pub(crate) fn resolve(&mut self, connection: &Connection, service: &str) -> Option<String> {
        if let Some(known) = self.0.get(service) {
            return Some(known.clone());
        }
        let found = read(connection, service)?;
        self.0.insert(service.to_owned(), found.clone());
        Some(found)
    }

    pub(crate) fn forget(&mut self, service: &str) {
        self.0.remove(service);
    }
}

fn read(connection: &Connection, service: &str) -> Option<String> {
    let name = BusName::try_from(service.to_owned()).ok()?;
    let bus = DBusProxy::new(connection).ok()?;
    program(bus.get_connection_unix_process_id(name).ok()?)
}

fn program(pid: u32) -> Option<String> {
    let root = format!("/proc/{pid}");
    argv0(&root).or_else(|| comm(&root))
}

fn argv0(root: &str) -> Option<String> {
    let line = std::fs::read(format!("{root}/cmdline")).ok()?;
    let first = line.split(|byte| *byte == 0).next()?;
    let path = std::str::from_utf8(first).ok()?;
    let name = Path::new(path).file_name()?.to_str()?;
    (!name.is_empty()).then(|| name.to_owned())
}

fn comm(root: &str) -> Option<String> {
    let name = std::fs::read_to_string(format!("{root}/comm")).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}
