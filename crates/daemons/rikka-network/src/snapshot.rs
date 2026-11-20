use crate::access_point::{AccessPoint, ApMode};
use crate::active::Active;
use crate::device::{Device, DeviceId, DeviceKind};
use crate::profile::Profile;
use crate::ssid::Ssid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Connectivity {
    #[default]
    Unknown,
    None,
    Portal,
    Limited,
    Full,
}

impl Connectivity {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            1 => Self::None,
            2 => Self::Portal,
            3 => Self::Limited,
            4 => Self::Full,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Radio {
    pub enabled: bool,
    pub hardware: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub networking: bool,
    pub wireless: Radio,
    pub connectivity: Connectivity,
    pub hostname: String,
    pub devices: Vec<Device>,
    pub access_points: Vec<AccessPoint>,
    pub profiles: Vec<Profile>,
    pub active: Vec<Active>,
}

impl Snapshot {
    #[must_use]
    pub fn device(&self, id: &DeviceId) -> Option<&Device> {
        self.devices.iter().find(|device| &device.id == id)
    }

    pub fn devices_of(&self, kind: DeviceKind) -> impl Iterator<Item = &Device> {
        self.devices
            .iter()
            .filter(move |device| device.kind == kind)
    }

    #[must_use]
    pub fn profile(&self, uuid: &str) -> Option<&Profile> {
        self.profiles.iter().find(|profile| profile.uuid == uuid)
    }

    #[must_use]
    pub fn saved(&self, wireless: bool) -> Vec<&Profile> {
        let mut profiles: Vec<&Profile> = self
            .profiles
            .iter()
            .filter(|profile| profile.wireless.is_some() == wireless)
            .collect();
        profiles.sort_by(|left, right| {
            right
                .timestamp
                .cmp(&left.timestamp)
                .then_with(|| left.id.cmp(&right.id))
        });
        profiles
    }

    #[must_use]
    pub fn active_on(&self, id: &DeviceId) -> Option<&Active> {
        self.active
            .iter()
            .find(|active| active.devices.contains(id))
    }

    #[must_use]
    pub fn activation(&self, uuid: &str) -> Option<&Active> {
        self.active.iter().find(|active| active.uuid == uuid)
    }

    #[must_use]
    pub fn networks(&self, device: &DeviceId) -> Vec<&AccessPoint> {
        let mut best: Vec<&AccessPoint> = Vec::new();
        for point in self.access_points.iter().filter(|point| {
            &point.device == device
                && point.mode == ApMode::Infrastructure
                && !point.ssid.is_empty()
        }) {
            match best
                .iter_mut()
                .find(|known| known.ssid == point.ssid && known.security == point.security)
            {
                Some(known) if known.strength < point.strength || point.in_use => *known = point,
                Some(_) => {}
                None => best.push(point),
            }
        }
        best.sort_by(|left, right| {
            right
                .in_use
                .cmp(&left.in_use)
                .then_with(|| right.saved.is_some().cmp(&left.saved.is_some()))
                .then_with(|| right.strength.cmp(&left.strength))
                .then_with(|| left.ssid.cmp(&right.ssid))
        });
        best
    }

    #[must_use]
    pub fn saved_for(&self, ssid: &Ssid) -> Option<&Profile> {
        self.profiles.iter().find(|profile| {
            profile
                .wireless
                .as_ref()
                .is_some_and(|wireless| &wireless.ssid == ssid)
        })
    }
}
