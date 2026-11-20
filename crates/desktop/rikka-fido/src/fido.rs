use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::authenticator::Authenticator;
use crate::ceremony::{Ceremony, Watch};
use crate::error::{Error, Result};
use crate::flow;
use crate::hid::{Device, Hidraw, Transport};
use crate::webauthn::{Assertion, AssertionRequest, Registration, RegistrationRequest};

const OPEN_TIME: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct Fido {
    transport: Arc<dyn Transport>,
}

impl Fido {
    #[must_use]
    pub fn new() -> Fido {
        Fido::with_transport(Hidraw::new())
    }

    #[must_use]
    pub fn with_transport(transport: impl Transport + 'static) -> Fido {
        Fido {
            transport: Arc::new(transport),
        }
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        self.transport.devices().map_err(Error::io)
    }

    pub fn open(&self, device: &Device) -> Result<Authenticator> {
        let link = self.transport.open(device).map_err(|error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                Error::AccessDenied(device.path().to_owned())
            } else {
                Error::io(error)
            }
        })?;
        Authenticator::connect(device.clone(), link, &Watch::bounded(OPEN_TIME))
    }

    pub fn assert(&self, request: AssertionRequest) -> Result<Ceremony<Assertion>> {
        let request = request.prepare()?;
        let transport = self.transport.clone();
        Ceremony::start(request.timeout, move |watch, prompter| {
            flow::assert::run(transport.as_ref(), &request, watch, prompter)
        })
    }

    pub fn register(&self, request: RegistrationRequest) -> Result<Ceremony<Registration>> {
        let request = request.prepare()?;
        let transport = self.transport.clone();
        Ceremony::start(request.timeout, move |watch, prompter| {
            flow::register::run(transport.as_ref(), &request, watch, prompter)
        })
    }
}

impl Default for Fido {
    fn default() -> Fido {
        Fido::new()
    }
}

impl fmt::Debug for Fido {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fido").finish_non_exhaustive()
    }
}
