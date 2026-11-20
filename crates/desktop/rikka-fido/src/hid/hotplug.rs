use std::io;
use std::time::Duration;

use rikka_types::CancelToken;

pub trait Hotplug {
    fn wait(&mut self, cancel: &CancelToken, timeout: Duration) -> io::Result<()>;
}

#[derive(Debug, Default)]
pub(crate) struct StaticDevices;

impl Hotplug for StaticDevices {
    fn wait(&mut self, cancel: &CancelToken, timeout: Duration) -> io::Result<()> {
        let _cancelled = cancel.wait_timeout(timeout);
        Ok(())
    }
}
