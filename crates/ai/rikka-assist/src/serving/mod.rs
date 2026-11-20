mod host;
mod seats;
mod service;

use rikka_sync::{Mutex, MutexGuard};
use std::sync::{Arc, OnceLock, Weak};

use rikka_seam::Publisher;
use rikka_tool::Function;

use crate::error::{Error, Result};
use crate::offering::Offering;
use crate::protocol::CHANGED;
use host::Host;
use seats::Seats;

static OFFERED: Mutex<Weak<Registration>> = Mutex::new(Weak::new());

#[derive(Debug)]
struct Registration {
    shared: Arc<Shared>,
    _host: Host,
}

#[derive(Debug)]
pub(crate) struct Shared {
    offering: Offering,
    seats: Mutex<Seats>,
    publisher: OnceLock<Publisher>,
}

#[must_use = "a window is offered only while its seat is held"]
#[derive(Debug)]
pub struct ToolRegistration {
    id: u64,
    registration: Arc<Registration>,
}

impl ToolRegistration {
    pub fn register(
        offering: Offering,
        title: impl Into<String>,
        functions: Vec<Function>,
    ) -> Result<ToolRegistration> {
        for function in &functions {
            service::unwindowed(function)?;
        }
        let registration = Registration::register(offering)?;
        let title = title.into();
        let id = registration
            .shared
            .reseat(|seats| seats.add(title, functions));
        Ok(ToolRegistration { id, registration })
    }

    pub fn set_title(&self, title: impl Into<String>) {
        self.registration
            .shared
            .seats()
            .retitle(self.id, title.into());
    }

    pub fn mark_active(&self) {
        self.registration.shared.reseat(|seats| seats.used(self.id));
    }
}

impl Drop for ToolRegistration {
    fn drop(&mut self) {
        self.registration
            .shared
            .reseat(|seats| seats.remove(self.id));
    }
}

impl Registration {
    fn register(offering: Offering) -> Result<Arc<Registration>> {
        let mut offerings = OFFERED.lock();
        if let Some(registry) = offerings.upgrade() {
            return match registry.shared.offering.kit() == offering.kit() {
                true => Ok(registry),
                false => Err(Error::Offered {
                    offered: registry.shared.offering.kit().to_owned(),
                    requested: offering.kit().to_owned(),
                }),
            };
        }

        let shared = Arc::new(Shared {
            offering,
            seats: Mutex::new(Seats::default()),
            publisher: OnceLock::new(),
        });
        let registry = Arc::new(Registration {
            _host: Host::start(Arc::clone(&shared))?,
            shared,
        });
        *offerings = Arc::downgrade(&registry);
        Ok(registry)
    }
}

impl Shared {
    pub(crate) fn offering(&self) -> &Offering {
        &self.offering
    }

    pub(crate) fn seats(&self) -> MutexGuard<'_, Seats> {
        self.seats.lock()
    }

    pub(crate) fn publish_with(&self, publisher: Publisher) {
        let _ = self.publisher.set(publisher);
    }

    fn reseat<T>(&self, change: impl FnOnce(&mut Seats) -> T) -> T {
        let (changed, moved) = {
            let mut seats = self.seats();
            let before = seats.fronted();
            let changed = change(&mut seats);
            (changed, !seats.fronts(&before))
        };
        if moved
            && let Some(publisher) = self.publisher.get()
            && let Err(error) = publisher.emit(CHANGED, &())
        {
            tracing::debug!("failed to announce tool changes: {error}");
        }
        changed
    }
}
