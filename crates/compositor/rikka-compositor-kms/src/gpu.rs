use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const DEFAULT_SEAT: &str = "seat0";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Gpu {
    path: PathBuf,
    name: String,
    number: u64,
    boots: bool,
}

impl Gpu {
    pub fn all(seat: &str) -> Result<Vec<Self>> {
        let mut enumerator = udev::Enumerator::new().map_err(Error::Udev)?;
        enumerator.match_subsystem("drm").map_err(Error::Udev)?;
        enumerator
            .match_sysname("card[0-9]*")
            .map_err(Error::Udev)?;
        let mut gpus: Vec<Self> = enumerator
            .scan_devices()
            .map_err(Error::Udev)?
            .filter(|device| is_card(device) && on_seat(device, seat))
            .filter_map(|device| Self::from_udev(&device))
            .collect();
        if gpus.is_empty() {
            return Err(Error::NoGpu(seat.to_owned()));
        }
        gpus.sort_by_key(|gpu| (!gpu.boots, gpu.number));
        Ok(gpus)
    }

    pub(crate) fn from_udev(device: &udev::Device) -> Option<Self> {
        let path = device.devnode()?.to_owned();
        let number = device.devnum()?;
        let boots = device
            .parent_with_subsystem("pci")
            .ok()
            .flatten()
            .and_then(|pci| pci.attribute_value("boot_vga").map(|value| value == "1"))
            .unwrap_or(false);
        Some(Self {
            path,
            name: device.sysname().to_string_lossy().into_owned(),
            number,
            boots,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn device_number(&self) -> u64 {
        self.number
    }

    pub fn major(&self) -> u32 {
        rustix::fs::major(self.number)
    }

    pub fn minor(&self) -> u32 {
        rustix::fs::minor(self.number)
    }
}

pub(crate) fn on_seat(device: &udev::Device, seat: &str) -> bool {
    device
        .property_value("ID_SEAT")
        .unwrap_or(OsStr::new(DEFAULT_SEAT))
        == seat
}

pub(crate) fn is_card(device: &udev::Device) -> bool {
    device.sysname().to_string_lossy().starts_with("card")
        && !device.sysname().to_string_lossy().contains('-')
}
