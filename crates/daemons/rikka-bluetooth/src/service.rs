use std::sync::Arc;

use rikka_bus::{Backend, EventSink, Service, StartError, Unavailable};

use crate::agent::Prompts;
use crate::event::Event;
use crate::prompt::Reply;
use crate::request::Request;
use crate::session::{Agency, Bluez, Kit};
use crate::state::State;

#[derive(Debug)]
pub(crate) struct Session {
    service: Service<Bluez>,
    prompts: Arc<Prompts>,
}

impl Session {
    pub(crate) fn start(
        report: EventSink<Event>,
        unavailable: Unavailable,
        agency: Agency,
    ) -> Result<Session, StartError> {
        let prompts = Arc::new(Prompts::new(report.clone()));
        let kit = Kit {
            agency,
            prompts: Arc::clone(&prompts),
        };
        let service = Service::start(kit, report, |error| unavailable.report(error))?;
        Ok(Session { service, prompts })
    }

    pub(crate) fn answer(&self, prompt: u64, reply: Reply) {
        self.prompts.answer(prompt, reply);
    }
}

impl Backend for Session {
    type Request = Request;
    type Event = Event;
    type State = State;

    const UNAVAILABLE: &'static str = "this machine has no Bluetooth";

    fn start(report: EventSink<Event>, unavailable: Unavailable) -> Result<Session, StartError> {
        Session::start(report, unavailable, Agency::Watches)
    }

    fn request(&self, request: Request) {
        self.service.send(request);
    }
}
