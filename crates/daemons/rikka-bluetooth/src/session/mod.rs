mod act;
mod proxies;
mod read;

use std::sync::Arc;

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::ObjectPath;

use crate::agent::{self, Agent, Prompts};
use crate::event::Event;
use crate::request::Request;
use crate::session::proxies::AgentManagerProxy;
use crate::state::State;
use rikka_bus::{EventSink, Handler, ManagedObjects, Notifier, ObjectChanges, StartError, Watch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Agency {
    Answers,
    Watches,
}

pub(crate) struct Kit {
    pub(crate) agency: Agency,
    pub(crate) prompts: Arc<Prompts>,
}

pub(crate) struct Bluez {
    connection: Connection,
    objects: ManagedObjects,
    prompts: Arc<Prompts>,
    _watch: Watch,
}

impl Handler for Bluez {
    type Config = Kit;
    type Request = Request;
    type Notice = ObjectChanges;
    type Event = Event;
    type State = State;

    const THREAD_NAME: &'static str = "rikka-bluez";
    const UNAVAILABLE: &'static str = "this machine has no Bluetooth";

    fn open(
        kit: Kit,
        notifier: Notifier<Bluez>,
        report: &EventSink<Event>,
    ) -> Result<Bluez, StartError> {
        let connection = serve(Arc::clone(&kit.prompts))?;
        let watch = ManagedObjects::watch(
            &connection,
            proxies::SERVICE,
            proxies::ROOT,
            move |changes| notifier.notify(changes),
        )?;
        let objects = ManagedObjects::read(&connection, proxies::SERVICE, proxies::ROOT)?;
        report.emit(Event::Changed(Box::new(read::snapshot(&objects))));

        if kit.agency == Agency::Answers
            && let Err(error) = register(&connection)
        {
            tracing::warn!("failed to register BlueZ agent: {error}");
        }

        Ok(Bluez {
            connection,
            objects,
            prompts: kit.prompts,
            _watch: watch,
        })
    }

    fn handle_request(&mut self, request: Request, report: &EventSink<Event>) {
        if request.is_slow() {
            self.wait_on(request, report);
            return;
        }
        let (action, device) = (request.action(), request.device().cloned());
        let error = act::perform(&self.connection, request)
            .err()
            .map(|error| error.to_string());
        report.emit(Event::Answered {
            action,
            device,
            error,
        });
    }

    fn handle_notice(&mut self, changes: ObjectChanges, report: &EventSink<Event>) {
        match self.objects.follow(&self.connection, changes) {
            Ok(true) => report.emit(Event::Changed(Box::new(read::snapshot(&self.objects)))),
            Ok(false) => {}
            Err(error) => tracing::debug!("failed to follow BlueZ: {error}"),
        }
    }
}

impl Bluez {
    fn wait_on(&self, request: Request, report: &EventSink<Event>) {
        let (action, device) = (request.action(), request.device().cloned());
        let prompts = Arc::clone(&self.prompts);
        let answering = report.clone();
        let started = std::thread::Builder::new()
            .name("rikka-bluez-wait".to_owned())
            .spawn(move || {
                let error = match Connection::system() {
                    Ok(connection) => act::perform(&connection, request)
                        .err()
                        .map(|error| error.to_string()),
                    Err(error) => Some(error.to_string()),
                };
                prompts.withdraw();
                answering.emit(Event::Answered {
                    action,
                    device,
                    error,
                });
            });
        if let Err(error) = started {
            report.emit(Event::Answered {
                action,
                device: None,
                error: Some(error.to_string()),
            });
        }
    }
}

fn serve(prompts: Arc<Prompts>) -> zbus::Result<Connection> {
    Builder::system()?
        .serve_at(agent::PATH, Agent::new(prompts))?
        .build()
}

fn register(connection: &Connection) -> zbus::Result<()> {
    let path = ObjectPath::try_from(agent::PATH)?;
    let manager = AgentManagerProxy::new(connection)?;
    manager.register_agent(&path, agent::CAPABILITY)?;
    manager.request_default_agent(&path)
}
