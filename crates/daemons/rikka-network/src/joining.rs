use crate::device::{DeviceId, DeviceState};
use crate::failure::Failure;
use crate::ssid::Ssid;
use crate::state::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Unjoinable {
    #[error("no Wi-Fi device")]
    NoRadio,
    #[error("Wi-Fi is turned off")]
    RadioOff,
    #[error("network requires enterprise credentials not carried by the code")]
    NeedsAccount,
    #[error("network is out of range")]
    OutOfRange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    Pending,
    Joined,
    Failed(Failure),
    Refused(String),
}

#[derive(Clone, Debug)]
pub struct Joining {
    ssid: Ssid,
    device: DeviceId,
    began: bool,
}

impl Joining {
    pub(crate) fn new(ssid: Ssid, device: DeviceId) -> Joining {
        Joining {
            ssid,
            device,
            began: false,
        }
    }

    #[must_use]
    pub fn ssid(&self) -> &Ssid {
        &self.ssid
    }

    pub fn follow(&mut self, state: &State) -> Progress {
        if let Some(error) = state
            .error
            .as_ref()
            .filter(|error| error.action.concerns_join())
        {
            return Progress::Refused(error.reason.clone());
        }
        let Some(device) = state.snapshot.device(&self.device) else {
            return Progress::Failed(Failure::Missing);
        };
        let joined = state.joined().is_some_and(|point| point.ssid == self.ssid);
        match device.state {
            DeviceState::Connected if joined => Progress::Joined,
            DeviceState::Connecting | DeviceState::NeedsSecrets => {
                self.began = true;
                Progress::Pending
            }
            DeviceState::Failed if self.began => Progress::Failed(device.failure),
            DeviceState::Disconnected if self.began && device.failure.happened() => {
                Progress::Failed(device.failure)
            }
            _ => Progress::Pending,
        }
    }
}
