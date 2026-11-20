use std::io;
use std::os::fd::BorrowedFd;
use std::time::Duration;

use super::device::Device;
use super::hotplug::{Hotplug, StaticDevices};

pub trait HidLink: Send {
    fn packet_len(&self) -> usize;

    fn send(&mut self, packet: &[u8]) -> io::Result<()>;

    fn receive(
        &mut self,
        wait: Duration,
        interrupt: Option<BorrowedFd<'_>>,
    ) -> io::Result<Option<Vec<u8>>>;
}

pub trait Transport: Send + Sync {
    fn devices(&self) -> io::Result<Vec<Device>>;

    fn open(&self, device: &Device) -> io::Result<Box<dyn HidLink>>;

    fn hotplug(&self) -> io::Result<Box<dyn Hotplug>> {
        Ok(Box::new(StaticDevices))
    }
}
