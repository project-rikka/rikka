use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use zbus::message::Message;
use zbus::zvariant::OwnedValue;

use crate::names::{PATH, PLAYER, PREFIX, PROPERTIES, ROOT};
use crate::watch::session::Mpris;
use rikka_bus::{Coalesce, Notifier, Rule, Signals, Watch};
use zbus::blocking::Connection;

const SOON: Duration = Duration::from_millis(30);
const LATEST: Duration = Duration::from_millis(200);

const BUS: &str = "org.freedesktop.DBus";

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Changed {
    pub(crate) root: bool,
    pub(crate) player: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Changes {
    pub(crate) owners: BTreeMap<String, String>,
    pub(crate) changed: BTreeMap<String, Changed>,
    pub(crate) seeked: BTreeMap<String, i64>,
}

impl Coalesce for Changes {
    fn merge(&mut self, other: Changes) {
        self.owners.extend(other.owners);
        for (sender, changed) in other.changed {
            let entry = self.changed.entry(sender).or_default();
            entry.root |= changed.root;
            entry.player |= changed.player;
        }
        self.seeked.extend(other.seeked);
    }
}

pub(crate) fn listen(connection: &Connection, notifier: Notifier<Mpris>) -> zbus::Result<Watch> {
    Signals::new()
        .debounce(SOON, LATEST)
        .rule(
            Rule::new()
                .interface(BUS)
                .member("NameOwnerChanged")
                .arg0_namespace(PREFIX.trim_end_matches('.')),
        )
        .rule(
            Rule::new()
                .interface(PROPERTIES)
                .member("PropertiesChanged")
                .path(PATH),
        )
        .rule(Rule::new().interface(PLAYER).member("Seeked").path(PATH))
        .listen(connection, sort, move |notice| notifier.notify(notice))
}

fn sort(message: &Message) -> Option<Changes> {
    let header = message.header();
    let member = header.member()?.as_str();
    let mut notice = Changes::default();
    match (header.interface()?.as_str(), member) {
        (BUS, "NameOwnerChanged") => {
            let (name, _, owner): (String, String, String) = message.body().deserialize().ok()?;
            if !name.starts_with(PREFIX) {
                return None;
            }
            notice.owners.insert(name, owner);
        }
        (PROPERTIES, "PropertiesChanged") => {
            type Body = (String, HashMap<String, OwnedValue>, Vec<String>);
            let (interface, _, _): Body = message.body().deserialize().ok()?;
            let changed = Changed {
                root: interface == ROOT,
                player: interface == PLAYER,
            };
            if !changed.root && !changed.player {
                return None;
            }
            notice.changed.insert(header.sender()?.to_string(), changed);
        }
        (PLAYER, "Seeked") => {
            let position: i64 = message.body().deserialize().ok()?;
            notice.seeked.insert(header.sender()?.to_string(), position);
        }
        _ => return None,
    }
    Some(notice)
}
