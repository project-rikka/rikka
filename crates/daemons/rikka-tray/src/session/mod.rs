mod act;
mod menu;
mod programs;
mod read;
mod watch;

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::blocking::fdo::DBusProxy;
use zbus::names::BusName;

use crate::event::Event;
use crate::id::ItemId;
use crate::item::Item;
use crate::proxies::{HOST_PREFIX, WATCHER, WATCHER_PATH, WatcherProxy};
use crate::registry::Registry;
use crate::request::Request;
use crate::session::programs::Programs;
use crate::state::State;
use crate::watcher::Watcher;
use rikka_bus::{EventSink, Handler, Notifier, StartError, Watch};

pub(crate) use watch::Changes;

const PATIENCE: Duration = Duration::from_secs(4);

pub(crate) struct Trays {
    connection: Connection,
    serving: Option<Serving>,
    programs: Programs,
    owners: HashMap<String, String>,
    items: Vec<Item>,
    open: Option<ItemId>,
    _signals: Option<Watch>,
}

struct Serving {
    connection: Connection,
    registry: Arc<Registry>,
}

impl Handler for Trays {
    type Config = ();
    type Request = Request;
    type Notice = Changes;
    type Event = Event;
    type State = State;

    const THREAD_NAME: &'static str = "rikka-tray";
    const UNAVAILABLE: &'static str = "this session shows no tray";

    fn open(
        (): (),
        notifier: Notifier<Trays>,
        report: &EventSink<Event>,
    ) -> Result<Trays, StartError> {
        let connection = client()?;
        let serving = serve(notifier.clone());
        host(&connection)?;

        let signals = watch::watch(&connection, notifier)
            .inspect_err(|error| tracing::warn!("failed to watch tray signals: {error}"))
            .ok();
        let mut trays = Trays {
            connection,
            serving,
            programs: Programs::default(),
            owners: HashMap::new(),
            items: Vec::new(),
            open: None,
            _signals: signals,
        };
        trays.reread(report);
        Ok(trays)
    }

    fn handle_request(&mut self, request: Request, report: &EventSink<Event>) {
        let Some(item) = find(&self.items, request.item()).cloned() else {
            return;
        };
        self.open = match &request {
            Request::Menu(_) => match act::perform(&self.connection, &item, &request) {
                Some(menu) => {
                    let menu_item = menu.item.clone();
                    report.emit(Event::Opened(Box::new(menu)));
                    Some(menu_item)
                }
                None => {
                    report.emit(Event::Closed);
                    None
                }
            },
            Request::Choose { .. } => {
                act::perform(&self.connection, &item, &request);
                act::perform(&self.connection, &item, &Request::Closed(item.id.clone()));
                report.emit(Event::Closed);
                None
            }
            Request::Closed(_) => {
                act::perform(&self.connection, &item, &request);
                report.emit(Event::Closed);
                None
            }
            press => {
                act::perform(&self.connection, &item, press);
                self.open.clone()
            }
        };
    }

    fn handle_notice(&mut self, notice: Changes, report: &EventSink<Event>) {
        for item in notice.registered {
            self.announce("StatusNotifierItemRegistered", &item.to_string());
        }
        for service in notice.hosted {
            self.announce("StatusNotifierHostRegistered", &());
            tracing::debug!("tray host registered: {service}");
        }
        let dropped = self.dropped(&notice.gone);
        for item in &dropped {
            self.announce("StatusNotifierItemUnregistered", &item.to_string());
        }
        let lost = !dropped.is_empty() || notice.gone.iter().any(|gone| self.shows(gone));
        if notice.items || lost {
            self.reread(report);
        } else if !notice.changed.is_empty() {
            self.refresh(&notice.changed, report);
        }
        if notice.menu {
            self.reopen(report);
        }
    }
}

impl Trays {
    fn reread(&mut self, report: &EventSink<Event>) {
        let ids = match &self.serving {
            Some(serving) => serving.registry.items(),
            None => listed(&self.connection),
        };
        let items: Vec<Item> = ids
            .iter()
            .filter_map(|id| read::item(&self.connection, &mut self.programs, id))
            .collect();
        self.owners = items
            .iter()
            .filter_map(|item| {
                let service = &item.id.service;
                Some((service.clone(), owner(&self.connection, service)?))
            })
            .collect();
        self.publish(items, report);
    }

    fn refresh(&mut self, senders: &BTreeSet<String>, report: &EventSink<Event>) {
        let mut items = std::mem::take(&mut self.items);
        let Trays {
            connection,
            programs,
            owners,
            ..
        } = self;
        let mut moved = false;
        items.retain_mut(|item| {
            let service = &item.id.service;
            let sender = owners.get(service).unwrap_or(service);
            if !senders.contains(sender) {
                return true;
            }
            moved = true;
            match read::item(connection, programs, &item.id) {
                Some(fresh) => {
                    *item = fresh;
                    true
                }
                None => false,
            }
        });
        match moved {
            true => self.publish(items, report),
            false => self.items = items,
        }
    }

    fn publish(&mut self, mut items: Vec<Item>, report: &EventSink<Event>) {
        items.sort_by(|left, right| {
            (left.category, left.name.as_str(), &left.id).cmp(&(
                right.category,
                right.name.as_str(),
                &right.id,
            ))
        });
        self.items = items;
        report.emit(Event::Changed(self.items.clone()));

        if self
            .open
            .as_ref()
            .is_some_and(|id| find(&self.items, id).is_none())
        {
            self.open = None;
            report.emit(Event::Closed);
        }
    }

    fn shows(&self, service: &str) -> bool {
        self.items.iter().any(|item| {
            item.id.service == service
                || self
                    .owners
                    .get(&item.id.service)
                    .is_some_and(|owner| owner == service)
        })
    }

    fn reopen(&self, report: &EventSink<Event>) {
        let Some(item) = self.open.as_ref().and_then(|id| find(&self.items, id)) else {
            return;
        };
        match menu::read(&self.connection, item) {
            Some(menu) => report.emit(Event::Opened(Box::new(menu))),
            None => report.emit(Event::Closed),
        }
    }

    fn dropped(&mut self, gone: &[String]) -> Vec<ItemId> {
        let mut dropped = Vec::new();
        for service in gone {
            self.programs.forget(service);
            if let Some(serving) = &self.serving {
                dropped.extend(serving.registry.drop_owner(service));
            }
        }
        dropped
    }

    fn announce<B>(&self, signal: &str, body: &B)
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        let Some(serving) = &self.serving else {
            return;
        };
        let sent =
            serving
                .connection
                .emit_signal(None::<&str>, WATCHER_PATH, WATCHER, signal, body);
        if let Err(error) = sent {
            tracing::warn!("failed to emit `{signal}`: {error}");
        }
    }
}

fn client() -> zbus::Result<Connection> {
    Builder::session()?.method_timeout(PATIENCE).build()
}

fn serve(notifier: Notifier<Trays>) -> Option<Serving> {
    let registry = Arc::new(Registry::default());
    let connection = Builder::session()
        .ok()?
        .replace_existing_names(false)
        .allow_name_replacements(false)
        .name(WATCHER)
        .ok()?
        .serve_at(WATCHER_PATH, Watcher::new(Arc::clone(&registry), notifier))
        .ok()?
        .build()
        .inspect_err(|error| {
            tracing::debug!("another StatusNotifierWatcher owns the bus name: {error}")
        })
        .ok()?;
    tracing::info!("serving {WATCHER}");
    Some(Serving {
        connection,
        registry,
    })
}

fn host(connection: &Connection) -> zbus::Result<()> {
    let name = format!("{HOST_PREFIX}{}", std::process::id());
    connection.request_name(name.as_str())?;
    WatcherProxy::new(connection)?.register_status_notifier_host(&name)
}

fn listed(connection: &Connection) -> Vec<ItemId> {
    WatcherProxy::new(connection)
        .and_then(|watcher| watcher.registered_status_notifier_items())
        .inspect_err(|error| tracing::debug!("failed to list registered tray items: {error}"))
        .unwrap_or_default()
        .iter()
        .filter_map(|entry| ItemId::listed(entry))
        .collect()
}

fn find<'a>(items: &'a [Item], id: &ItemId) -> Option<&'a Item> {
    items.iter().find(|item| &item.id == id)
}

fn owner(connection: &Connection, service: &str) -> Option<String> {
    if service.starts_with(':') {
        return Some(service.to_owned());
    }
    let name = BusName::try_from(service).ok()?;
    let owner = DBusProxy::new(connection).ok()?.get_name_owner(name).ok()?;
    Some(owner.to_string())
}
