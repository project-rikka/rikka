use zbus::blocking::Connection;

use crate::action::{Action, Operation, PowerCapabilities, permits};
use crate::backlight::Backlight;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::request::Request;
use crate::state::State;
use std::convert::Infallible;

use rikka_bus::login::{ManagerProxy, current_session};
use rikka_bus::{DaemonError, EventSink, Handler, Notifier, StartError};

pub(crate) struct Logind {
    manager: ManagerProxy<'static>,
    backlight: Option<Backlight>,
}

impl Handler for Logind {
    type Config = ();
    type Request = Request;
    type Notice = Infallible;
    type Event = Event;
    type State = State;

    const THREAD_NAME: &'static str = "rikka-logind";
    const UNAVAILABLE: &'static str = "this desktop cannot reach logind";

    fn open(
        (): (),
        _stirs: Notifier<Logind>,
        report: &EventSink<Event>,
    ) -> Result<Logind, StartError> {
        let logind = open()?;
        report.emit(Event::Started {
            capabilities: logind.capabilities(),
            brightness: logind.brightness(),
        });
        Ok(logind)
    }

    fn handle_request(&mut self, request: Request, report: &EventSink<Event>) {
        match request {
            Request::Ask(action) => report.emit(Event::Answered {
                operation: Operation::Ask(action),
                error: failure(self.ask(action)),
            }),
            Request::Brighten(level) => {
                let Some(backlight) = &self.backlight else {
                    return;
                };
                report.emit(Event::Answered {
                    operation: Operation::Brighten,
                    error: failure(backlight.set_level(level)),
                });
            }
            Request::Reread => report.emit(Event::Brightness(self.brightness())),
        }
    }

    fn handle_notice(&mut self, stir: Infallible, _report: &EventSink<Event>) {
        match stir {}
    }

    fn replaces(pending: &Request, next: &Request) -> bool {
        matches!(
            (pending, next),
            (Request::Brighten(_), Request::Brighten(_))
        )
    }
}

impl Logind {
    fn capabilities(&self) -> PowerCapabilities {
        let mut capabilities = PowerCapabilities::default();
        for action in Action::ALL {
            let answer = match action {
                Action::PowerOff => self.manager.can_power_off(),
                Action::Reboot => self.manager.can_reboot(),
                Action::Suspend => self.manager.can_suspend(),
            };
            match answer {
                Ok(answer) => capabilities.allow(action, permits(&answer)),
                Err(error) => {
                    tracing::debug!("failed to query logind for {action:?}: {error}");
                }
            }
        }
        match self.manager.can_hibernate() {
            Ok(answer) => capabilities.allow_hibernate(permits(&answer)),
            Err(error) => tracing::debug!("failed to query logind for hibernation: {error}"),
        }
        capabilities
    }

    fn ask(&self, action: Action) -> Result<()> {
        match action {
            Action::PowerOff => self.manager.power_off(true),
            Action::Reboot => self.manager.reboot(true),
            Action::Suspend => self.manager.suspend(true),
        }?;
        Ok(())
    }

    fn brightness(&self) -> Option<f32> {
        self.backlight
            .as_ref()?
            .level()
            .inspect_err(|error| tracing::debug!(%error, "failed to read backlight"))
            .ok()
    }
}

fn open() -> Result<Logind> {
    let connection = Connection::system().map_err(Error::unreachable)?;
    let manager = ManagerProxy::new(&connection).map_err(Error::unreachable)?;
    let session =
        current_session(&manager).map_err(|error| Error::Sessionless(error.to_string()))?;
    let backlight = Backlight::found(&connection, session);
    Ok(Logind { manager, backlight })
}

fn failure(done: Result<()>) -> Option<String> {
    done.err().map(|error| {
        tracing::warn!("logind: {error}");
        error.to_string()
    })
}
