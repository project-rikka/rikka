use std::collections::BTreeSet;
use std::time::Duration;

use zbus::blocking::Connection;
use zbus::message::Message;

use crate::id::ItemId;
use crate::proxies::{ITEM, ITEM_ALTERNATIVE, ITEM_SIGNALS, MENU, WATCHER};
use crate::session::Trays;
use rikka_bus::{Coalesce, Notifier, Rule, Signals, Watch};

const SOON: Duration = Duration::from_millis(60);
const LATEST: Duration = Duration::from_secs(1);

const BUS: &str = "org.freedesktop.DBus";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

#[derive(Debug, Default)]
pub(crate) struct Changes {
    pub(crate) items: bool,
    pub(crate) changed: BTreeSet<String>,
    pub(crate) menu: bool,
    pub(crate) registered: Vec<ItemId>,
    pub(crate) hosted: Vec<String>,
    pub(crate) gone: Vec<String>,
}

impl Changes {
    pub(crate) fn items() -> Changes {
        Changes {
            items: true,
            ..Changes::default()
        }
    }

    fn changed(sender: String) -> Changes {
        Changes {
            changed: BTreeSet::from([sender]),
            ..Changes::default()
        }
    }

    pub(crate) fn menu() -> Changes {
        Changes {
            menu: true,
            ..Changes::default()
        }
    }

    pub(crate) fn registered(item: ItemId) -> Changes {
        Changes {
            registered: vec![item],
            ..Changes::items()
        }
    }

    pub(crate) fn hosted(service: String) -> Changes {
        Changes {
            hosted: vec![service],
            ..Changes::items()
        }
    }

    fn gone(service: String) -> Changes {
        Changes {
            gone: vec![service],
            ..Changes::default()
        }
    }
}

impl Coalesce for Changes {
    fn merge(&mut self, other: Changes) {
        self.items |= other.items;
        self.changed.extend(other.changed);
        self.menu |= other.menu;
        self.registered.extend(other.registered);
        self.hosted.extend(other.hosted);
        self.gone.extend(other.gone);
    }
}

pub(crate) fn watch(connection: &Connection, notifier: Notifier<Trays>) -> zbus::Result<Watch> {
    Signals::new()
        .debounce(SOON, LATEST)
        .rule(Rule::new().interface(ITEM))
        .rule(Rule::new().interface(ITEM_ALTERNATIVE))
        .rule(Rule::new().interface(WATCHER))
        .rule(Rule::new().interface(MENU))
        .rule(
            Rule::new()
                .interface(PROPERTIES)
                .member("PropertiesChanged")
                .arg(0, ITEM),
        )
        .rule(Rule::new().interface(BUS).member("NameOwnerChanged"))
        .listen(connection, sort, move |notice| notifier.notify(notice))
}

fn sort(message: &Message) -> Option<Changes> {
    let header = message.header();
    let interface = header.interface()?.as_str();
    let member = header.member()?.as_str();
    let sender = || header.sender().map(ToString::to_string);

    match interface {
        ITEM | ITEM_ALTERNATIVE if ITEM_SIGNALS.contains(&member) => sender().map(Changes::changed),
        PROPERTIES if member == "PropertiesChanged" => sender().map(Changes::changed),
        WATCHER if member.starts_with("StatusNotifierItem") => Some(Changes::items()),
        MENU => Some(Changes::menu()),
        BUS if member == "NameOwnerChanged" => released(message).map(Changes::gone),
        _ => None,
    }
}

fn released(message: &Message) -> Option<String> {
    let (name, old, _): (String, String, String) = message.body().deserialize().ok()?;
    (!old.is_empty()).then_some(name)
}
