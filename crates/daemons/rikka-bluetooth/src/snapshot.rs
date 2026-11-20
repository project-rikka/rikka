use crate::adapter::{Adapter, AdapterId};
use crate::device::{Device, DeviceId};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub adapters: Vec<Adapter>,
    pub devices: Vec<Device>,
}

impl Snapshot {
    #[must_use]
    pub fn adapter(&self, id: &AdapterId) -> Option<&Adapter> {
        self.adapters.iter().find(|adapter| &adapter.id == id)
    }

    #[must_use]
    pub fn device(&self, id: &DeviceId) -> Option<&Device> {
        self.devices.iter().find(|device| &device.id == id)
    }

    #[must_use]
    pub fn known(&self, adapter: &AdapterId) -> Vec<&Device> {
        let mut devices: Vec<&Device> = self
            .devices
            .iter()
            .filter(|device| &device.adapter == adapter && device.is_known())
            .collect();
        devices.sort_by(|left, right| {
            right
                .connected
                .cmp(&left.connected)
                .then_with(|| left.title.cmp(&right.title))
        });
        devices
    }

    #[must_use]
    pub fn nearby(&self, adapter: &AdapterId) -> Vec<&Device> {
        let mut devices: Vec<&Device> = self
            .devices
            .iter()
            .filter(|device| &device.adapter == adapter && !device.is_known())
            .collect();
        devices.sort_by(|left, right| {
            right
                .signal
                .cmp(&left.signal)
                .then_with(|| left.title.cmp(&right.title))
        });
        devices
    }
}
