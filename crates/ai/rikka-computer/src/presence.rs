use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::Duration;

use crate::environment::Environment;
use crate::session::exited_within;

#[derive(Debug)]
pub(crate) struct Presence {
    environment: Environment,
    compositor: Arc<OwnedFd>,
}

impl Presence {
    pub(crate) fn new(environment: Environment, compositor: Arc<OwnedFd>) -> Presence {
        Presence {
            environment,
            compositor,
        }
    }

    pub(crate) fn environment(&self) -> Option<Environment> {
        (!exited_within(&self.compositor, Duration::ZERO)).then(|| self.environment.clone())
    }
}
