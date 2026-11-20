use futures_util::Stream;

use crate::fix::{Accuracy, Fix};
use crate::session::{GeoClue, Request};
use rikka_bus::{Monitor, Service};

#[derive(Clone, Debug)]
pub struct Location(Monitor<Service<GeoClue>>);

impl Location {
    #[must_use]
    pub fn start(desktop_id: impl Into<String>, accuracy: Accuracy) -> Location {
        let request = Request {
            desktop: desktop_id.into(),
            accuracy,
        };
        Location(Monitor::start_with(move |report, unavailable| {
            Service::start(request, report, |error| unavailable.report(error))
        }))
    }

    #[must_use]
    pub fn state(&self) -> Option<Fix> {
        self.0.state()
    }

    pub fn changes(&self) -> impl Stream<Item = Option<Fix>> + Send + 'static {
        self.0.changes()
    }
}
