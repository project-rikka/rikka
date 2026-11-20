mod act;
pub(crate) mod proxies;
mod read;
pub(crate) mod user;

use std::collections::BTreeSet;

use zbus::blocking::Connection;

use crate::event::Event;
use crate::request::Request;
use crate::state::State;
use rikka_bus::{
    Coalesce, EventSink, Handler, ManagedObjects, Notifier, ObjectChanges, Rule, Signals,
    StartError, Watch,
};

const UPDATED: &str = "Updated";

pub(crate) struct Nm {
    connection: Connection,
    objects: ManagedObjects,
    saved: read::Saved,
    user: String,
    _watches: [Watch; 2],
}

pub(crate) enum Notice {
    Objects(ObjectChanges),
    Updated(Updated),
}

#[derive(Default)]
pub(crate) struct Updated(BTreeSet<String>);

impl Coalesce for Updated {
    fn merge(&mut self, other: Updated) {
        self.0.extend(other.0);
    }
}

impl Handler for Nm {
    type Config = ();
    type Request = Request;
    type Notice = Notice;
    type Event = Event;
    type State = State;

    const THREAD_NAME: &'static str = "rikka-nm";
    const UNAVAILABLE: &'static str = "this desktop has no network to show";

    fn open((): (), notifier: Notifier<Nm>, report: &EventSink<Event>) -> Result<Nm, StartError> {
        let connection = Connection::system()?;
        let watches = watch(&connection, notifier)?;
        let objects = ManagedObjects::read(&connection, proxies::SERVICE, proxies::ROOT)?;
        let mut nm = Nm {
            connection,
            objects,
            saved: read::Saved::new(),
            user: user::name(),
            _watches: watches,
        };
        nm.publish(report);
        Ok(nm)
    }

    fn handle_request(&mut self, request: Request, report: &EventSink<Event>) {
        let action = request.action();
        let error = match act::perform(&self.connection, request, &self.user) {
            Ok(revealed) => {
                if let Some(event) = revealed {
                    report.emit(event);
                }
                None
            }
            Err(error) => Some(error.to_string()),
        };
        report.emit(Event::Answered { action, error });
    }

    fn handle_notice(&mut self, notice: Notice, report: &EventSink<Event>) {
        match notice {
            Notice::Objects(changes) => match self.objects.follow(&self.connection, changes) {
                Ok(true) => self.publish(report),
                Ok(false) => {}
                Err(error) => tracing::debug!("failed to follow NetworkManager: {error}"),
            },
            Notice::Updated(Updated(paths)) => {
                self.saved.retain(|path, _| !paths.contains(path));
                self.publish(report);
            }
        }
    }
}

impl Nm {
    fn publish(&mut self, report: &EventSink<Event>) {
        let snapshot = read::snapshot(&self.connection, &self.objects, &mut self.saved);
        report.emit(Event::Changed(Box::new(snapshot)));
    }
}

fn watch(connection: &Connection, notifier: Notifier<Nm>) -> zbus::Result<[Watch; 2]> {
    let objects = notifier.clone();
    let tree = ManagedObjects::watch(
        connection,
        proxies::SERVICE,
        proxies::ROOT,
        move |changes| objects.notify(Notice::Objects(changes)),
    )?;
    let updates = Signals::new()
        .rule(
            Rule::new()
                .sender(proxies::SERVICE)
                .interface(proxies::CONNECTION)
                .member(UPDATED),
        )
        .listen(connection, updated, move |paths| {
            notifier.notify(Notice::Updated(paths))
        })?;
    Ok([tree, updates])
}

fn updated(message: &zbus::Message) -> Option<Updated> {
    let path = message.header().path()?.as_str().to_owned();
    Some(Updated(BTreeSet::from([path])))
}
